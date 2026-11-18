//! Which video file does the player have open? (no admin needed)
//!
//! Lists the player process's handles (`NtQuerySystemInformation`), duplicates each one
//! into KaiSync, keeps disk files, and asks Windows for their path. Runs on a helper
//! thread with a timeout, because a few handle kinds can make these calls block.

use std::{
    ffi::c_void,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

use windows::{
    Wdk::System::SystemInformation::{NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS},
    Win32::{
        Foundation::{CloseHandle, DuplicateHandle, DUPLICATE_SAME_ACCESS, HANDLE, HWND, LPARAM},
        Storage::FileSystem::{GetFileType, GetFinalPathNameByHandleW, GETFINALPATHNAMEBYHANDLE_FLAGS, FILE_NAME_NORMALIZED, FILE_TYPE_DISK, VOLUME_NAME_DOS},
        System::Threading::{GetCurrentProcess, OpenProcess, PROCESS_DUP_HANDLE},
        UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible},
    },
    core::BOOL,
};

const SYSTEM_EXTENDED_HANDLE_INFORMATION: i32 = 64;

/// Access masks of synchronous named-pipe handles. Querying those can block forever while
/// another thread waits on the pipe, so they are skipped (the same list handle tools use).
const PIPE_LIKE_ACCESS: &[u32] = &[0x0012_019F, 0x001A_019F, 0x0012_0189, 0x0010_0000, 0x0012_008D];

/// A lookup that got stuck is still running: don't start another one.
static BUSY: AtomicBool = AtomicBool::new(false);
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;

pub const VIDEO_EXTS: &[&str] = &[
    "mkv", "mp4", "m4v", "mov", "webm", "ts", "m2ts", "mts", "avi", "wmv", "flv", "hevc", "h265", "265", "mpg", "mpeg",
];

#[repr(C)]
struct HandleEntry {
    object: *mut c_void,
    unique_process_id: usize,
    handle_value: usize,
    granted_access: u32,
    creator_back_trace_index: u16,
    object_type_index: u16,
    handle_attributes: u32,
    reserved: u32,
}

pub fn is_video(p: &std::path::Path) -> bool {
    p.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|e| VIDEO_EXTS.contains(&e.as_str()))
}

/// All disk files `pid` has open (blocking; call through [`open_files`]).
fn open_files_blocking(pid: u32) -> Vec<PathBuf> {
    // 1. Snapshot every handle in the system (the table grows, so retry with a bigger buffer).
    let mut buf: Vec<u8> = vec![0; 4 << 20];
    loop {
        let mut needed = 0u32;
        let st = unsafe {
            NtQuerySystemInformation(
                SYSTEM_INFORMATION_CLASS(SYSTEM_EXTENDED_HANDLE_INFORMATION),
                buf.as_mut_ptr().cast(),
                buf.len() as u32,
                &mut needed,
            )
        };
        if st.0 == STATUS_INFO_LENGTH_MISMATCH {
            let next = (needed as usize).max(buf.len() * 2) + (1 << 20);
            if next > 256 << 20 {
                return Vec::new();
            }
            buf = vec![0; next];
            continue;
        }
        if st.0 < 0 {
            tracing::debug!("video: NtQuerySystemInformation failed: {:#x}", st.0);
            return Vec::new();
        }
        break;
    }

    let count = unsafe { *(buf.as_ptr() as *const usize) };
    let first = unsafe { buf.as_ptr().add(2 * std::mem::size_of::<usize>()) as *const HandleEntry };
    let max = (buf.len() - 2 * std::mem::size_of::<usize>()) / std::mem::size_of::<HandleEntry>();
    let entries = unsafe { std::slice::from_raw_parts(first, count.min(max)) };

    // 2. Duplicate the player's handles and keep the ones that are disk files.
    let Ok(proc_) = (unsafe { OpenProcess(PROCESS_DUP_HANDLE, false, pid) }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries
        .iter()
        .filter(|e| e.unique_process_id == pid as usize && !PIPE_LIKE_ACCESS.contains(&e.granted_access))
    {
        let mut dup = HANDLE::default();
        let ok = unsafe {
            DuplicateHandle(proc_, HANDLE(e.handle_value as *mut c_void), GetCurrentProcess(), &mut dup, 0, false, DUPLICATE_SAME_ACCESS)
        };
        if ok.is_err() {
            continue;
        }
        if unsafe { GetFileType(dup) } == FILE_TYPE_DISK {
            let mut name = vec![0u16; 1024];
            let n = unsafe { GetFinalPathNameByHandleW(dup, &mut name, GETFINALPATHNAMEBYHANDLE_FLAGS(FILE_NAME_NORMALIZED.0 | VOLUME_NAME_DOS.0)) } as usize;
            if n > 0 && n < name.len() {
                let s = String::from_utf16_lossy(&name[..n]);
                let s = s.strip_prefix(r"\\?\UNC\").map(|r| format!(r"\\{r}")).unwrap_or_else(|| s.trim_start_matches(r"\\?\").to_string());
                out.push(PathBuf::from(s));
            }
        }
        unsafe {
            let _ = CloseHandle(dup);
        }
    }
    unsafe {
        let _ = CloseHandle(proc_);
    }
    out
}

/// Open files of `pid`, or `None` if it took too long (or an earlier lookup is still stuck).
pub fn open_files(pid: u32) -> Option<Vec<PathBuf>> {
    if BUSY.swap(true, Ordering::AcqRel) {
        return None;
    }
    let (tx, rx) = mpsc::channel();
    let spawned = std::thread::Builder::new()
        .name("kaisync-open-files".into())
        .spawn(move || {
            let r = open_files_blocking(pid);
            BUSY.store(false, Ordering::Release);
            let _ = tx.send(r);
        });
    if spawned.is_err() {
        BUSY.store(false, Ordering::Release);
        return None;
    }
    let r = rx.recv_timeout(Duration::from_secs(3)).ok();
    if r.is_none() {
        tracing::warn!("video: listing the player's open files timed out");
    }
    r
}

/// Title of the process's main visible window (PotPlayer shows the file name there).
pub fn window_title(pid: u32) -> Option<String> {
    struct Ctx {
        pid: u32,
        title: Option<String>,
    }
    unsafe extern "system" fn cb(hwnd: HWND, lp: LPARAM) -> BOOL {
        let ctx = unsafe { &mut *(lp.0 as *mut Ctx) };
        let mut owner = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner)) };
        if owner == ctx.pid && unsafe { IsWindowVisible(hwnd) }.as_bool() {
            let mut buf = [0u16; 512];
            let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
            if n > 0 {
                ctx.title = Some(String::from_utf16_lossy(&buf[..n as usize]));
                return BOOL(0); // stop
            }
        }
        BOOL(1)
    }
    let mut ctx = Ctx { pid, title: None };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut Ctx as isize));
    }
    ctx.title
}

/// The video file the player is playing: an open video file, preferring the one whose
/// name is in the window title.
pub fn current_video(pid: u32) -> Option<PathBuf> {
    let files = open_files(pid)?;
    let mut videos: Vec<PathBuf> = files.into_iter().filter(|p| is_video(p)).collect();
    videos.sort();
    videos.dedup();
    if videos.len() > 1 {
        if let Some(title) = window_title(pid).map(|t| t.to_lowercase()) {
            if let Some(v) = videos.iter().find(|v| {
                v.file_name().is_some_and(|n| title.contains(&n.to_string_lossy().to_lowercase()))
            }) {
                return Some(v.clone());
            }
        }
    }
    videos.into_iter().next()
}

#[cfg(test)]
mod tests {
    /// Opens a real file and checks the handle walk finds it (on this test process).
    /// `KAISYNC_PROBE_FILES="x.mkv" cargo test open_files_self -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn open_files_self() {
        let path = std::env::var("KAISYNC_PROBE_FILES").expect("set KAISYNC_PROBE_FILES");
        let _keep = std::fs::File::open(&path).expect("open");
        let started = std::time::Instant::now();
        let files = super::open_files(std::process::id()).expect("timed out");
        let videos: Vec<_> = files.iter().filter(|p| super::is_video(p)).collect();
        println!("OPEN {} files, {} video(s) in {} ms: {:?}", files.len(), videos.len(), started.elapsed().as_millis(), videos);
        let want = std::path::Path::new(&path).file_name().unwrap().to_owned();
        assert!(videos.iter().any(|v| v.file_name() == Some(want.as_os_str())), "not found: {want:?}");
        println!("CURRENT {:?}", super::current_video(std::process::id()));
    }
}
