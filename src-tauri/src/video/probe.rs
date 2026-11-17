//! Is a video file HDR? Reads the colour metadata from the container header only
//! (MKV/WebM: Tracks > Video > Colour; MP4/MOV: stsd sample entry > colr/dvcC).
//! Falls back to the file name ("HDR", "HDR10", "HLG", "DV", "DoVi"…).

use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum HdrKind {
    #[serde(rename = "HDR10")]
    Hdr10,
    #[serde(rename = "HLG")]
    Hlg,
    #[serde(rename = "Dolby Vision")]
    DolbyVision,
    /// No metadata, but the file name says HDR.
    #[serde(rename = "HDR")]
    ByName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    Hdr(HdrKind),
    Sdr,
    /// No colour info found (treated as SDR unless the name says HDR).
    Unknown,
}

/// ITU-T H.273 transfer characteristics.
const TC_PQ: u64 = 16;
const TC_HLG: u64 = 18;
const TC_UNSPECIFIED: u64 = 2;

#[derive(Default, Debug)]
struct Colour {
    transfer: Option<u64>,
    mastering: bool,
    dolby_vision: bool,
}

impl Colour {
    fn verdict(&self) -> Probe {
        if self.dolby_vision {
            return Probe::Hdr(HdrKind::DolbyVision);
        }
        match self.transfer {
            Some(TC_PQ) => Probe::Hdr(HdrKind::Hdr10),
            Some(TC_HLG) => Probe::Hdr(HdrKind::Hlg),
            _ if self.mastering => Probe::Hdr(HdrKind::Hdr10),
            Some(TC_UNSPECIFIED) | None => Probe::Unknown,
            Some(_) => Probe::Sdr,
        }
    }
}

pub fn probe(path: &Path) -> Probe {
    let by_meta = match File::open(path) {
        Ok(f) => {
            let mut r = BufReader::with_capacity(64 * 1024, f);
            let mut magic = [0u8; 8];
            if r.read_exact(&mut magic).is_err() {
                Probe::Unknown
            } else {
                let _ = r.seek(SeekFrom::Start(0));
                if magic[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
                    mkv(&mut r).map(|c| c.verdict()).unwrap_or(Probe::Unknown)
                } else if &magic[4..8] == b"ftyp" || &magic[4..8] == b"moov" || &magic[4..8] == b"wide" || &magic[4..8] == b"free" {
                    mp4(&mut r).map(|c| c.verdict()).unwrap_or(Probe::Unknown)
                } else {
                    Probe::Unknown
                }
            }
        }
        Err(e) => {
            tracing::debug!("video: cannot open {}: {e}", path.display());
            Probe::Unknown
        }
    };
    match by_meta {
        Probe::Unknown if name_says_hdr(path) => Probe::Hdr(HdrKind::ByName),
        other => other,
    }
}

fn name_says_hdr(path: &Path) -> bool {
    let name = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|t| matches!(t, "hdr" | "hdr10" | "hdr10p" | "hdr10plus" | "hlg" | "dv" | "dovi" | "dolbyvision"))
        || name.contains("dolby vision")
}

// ---------------------------------------------------------------- MKV / WebM (EBML)

const EBML: u32 = 0x1A45DFA3;
const SEGMENT: u32 = 0x18538067;
const TRACKS: u32 = 0x1654AE6B;
const TRACK_ENTRY: u32 = 0xAE;
const TRACK_TYPE: u32 = 0x83;
const VIDEO: u32 = 0xE0;
const COLOUR: u32 = 0x55B0;
const TRANSFER: u32 = 0x55BA;
const MASTERING: u32 = 0x55D0;
const BLOCK_ADDITION_MAPPING: u32 = 0x41E4;
const BLOCK_ADD_ID_TYPE: u32 = 0x41E7;
const CLUSTER: u32 = 0x1F43B675;

fn read_byte<R: Read>(r: &mut R) -> Option<u8> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).ok()?;
    Some(b[0])
}

/// Element ID (marker bits kept).
fn read_id<R: Read>(r: &mut R) -> Option<u32> {
    let b = read_byte(r)?;
    let len = b.leading_zeros() as usize + 1;
    if len > 4 {
        return None;
    }
    let mut v = b as u32;
    for _ in 1..len {
        v = (v << 8) | read_byte(r)? as u32;
    }
    Some(v)
}

/// Element size; `None` inner value = unknown size.
fn read_size<R: Read>(r: &mut R) -> Option<Option<u64>> {
    let b = read_byte(r)?;
    let len = b.leading_zeros() as usize + 1;
    if len > 8 {
        return None;
    }
    let mut v = (b as u64) & (0xFF >> len);
    let mut all_ones = v == (0xFF >> len) as u64;
    for _ in 1..len {
        let n = read_byte(r)?;
        all_ones &= n == 0xFF;
        v = (v << 8) | n as u64;
    }
    Some(if all_ones { None } else { Some(v) })
}

fn read_uint<R: Read>(r: &mut R, size: u64) -> Option<u64> {
    if size > 8 {
        return None;
    }
    let mut v = 0u64;
    for _ in 0..size {
        v = (v << 8) | read_byte(r)? as u64;
    }
    Some(v)
}

/// Walk children of an element spanning [start, end), calling `f(id, size, reader)`.
/// `f` must leave the reader at the end of the child or return `false` to stop.
fn children<R: Read + Seek>(r: &mut R, end: u64, mut f: impl FnMut(&mut R, u32, u64) -> Option<bool>) -> Option<()> {
    loop {
        let pos = r.stream_position().ok()?;
        if pos >= end {
            return Some(());
        }
        let id = read_id(r)?;
        let size = read_size(r)?.unwrap_or(end.saturating_sub(r.stream_position().ok()?));
        let body = r.stream_position().ok()?;
        if !f(r, id, size)? {
            return Some(());
        }
        r.seek(SeekFrom::Start(body + size)).ok()?;
    }
}

fn mkv<R: Read + Seek>(r: &mut R) -> Option<Colour> {
    let file_end = r.seek(SeekFrom::End(0)).ok()?;
    r.seek(SeekFrom::Start(0)).ok()?;
    let mut found: Option<Colour> = None;
    children(r, file_end, |r, id, size| {
        if id == EBML {
            return Some(true);
        }
        if id != SEGMENT {
            return Some(true);
        }
        let seg_end = r.stream_position().ok()? + size;
        let mut clusters = 0;
        children(r, seg_end, |r, id, size| {
            if id == TRACKS {
                let tracks_end = r.stream_position().ok()? + size;
                found = mkv_tracks(r, tracks_end);
                return Some(false);
            }
            if id == CLUSTER {
                clusters += 1;
                // Tracks normally come before the first cluster; don't scan the whole file.
                return Some(clusters < 4);
            }
            Some(true)
        });
        Some(false)
    });
    found
}

fn mkv_tracks<R: Read + Seek>(r: &mut R, end: u64) -> Option<Colour> {
    let mut video: Option<Colour> = None;
    children(r, end, |r, id, size| {
        if id != TRACK_ENTRY {
            return Some(true);
        }
        let entry_end = r.stream_position().ok()? + size;
        let mut is_video = false;
        let mut c = Colour::default();
        children(r, entry_end, |r, id, size| {
            match id {
                TRACK_TYPE => is_video = read_uint(r, size)? == 1,
                VIDEO => {
                    let v_end = r.stream_position().ok()? + size;
                    children(r, v_end, |r, id, size| {
                        if id == COLOUR {
                            let c_end = r.stream_position().ok()? + size;
                            children(r, c_end, |r, id, size| {
                                match id {
                                    TRANSFER => c.transfer = read_uint(r, size),
                                    MASTERING => c.mastering = true,
                                    _ => {}
                                }
                                Some(true)
                            });
                        }
                        Some(true)
                    });
                }
                BLOCK_ADDITION_MAPPING => {
                    let m_end = r.stream_position().ok()? + size;
                    children(r, m_end, |r, id, size| {
                        if id == BLOCK_ADD_ID_TYPE {
                            let t = read_uint(r, size)?;
                            if t == u32::from_be_bytes(*b"dvcC") as u64 || t == u32::from_be_bytes(*b"dvvC") as u64 {
                                c.dolby_vision = true;
                            }
                        }
                        Some(true)
                    });
                }
                _ => {}
            }
            Some(true)
        });
        if is_video {
            video = Some(c);
            return Some(false);
        }
        Some(true)
    });
    video
}

// ---------------------------------------------------------------- MP4 / MOV (ISO BMFF)

const VIDEO_ENTRIES: &[&[u8; 4]] = &[
    b"avc1", b"avc3", b"hvc1", b"hev1", b"dvh1", b"dvhe", b"dvav", b"dva1", b"dav1", b"av01", b"vp09", b"mp4v",
];

/// Box header -> (type, body start, body end).
fn read_box<R: Read + Seek>(r: &mut R, parent_end: u64) -> Option<([u8; 4], u64, u64)> {
    let start = r.stream_position().ok()?;
    if start + 8 > parent_end {
        return None;
    }
    let mut h = [0u8; 8];
    r.read_exact(&mut h).ok()?;
    let mut size = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) as u64;
    let kind = [h[4], h[5], h[6], h[7]];
    let mut body = start + 8;
    if size == 1 {
        let mut l = [0u8; 8];
        r.read_exact(&mut l).ok()?;
        size = u64::from_be_bytes(l);
        body += 8;
    } else if size == 0 {
        size = parent_end - start;
    }
    if size < body - start {
        return None;
    }
    Some((kind, body, (start + size).min(parent_end)))
}

fn mp4<R: Read + Seek>(r: &mut R) -> Option<Colour> {
    let end = r.seek(SeekFrom::End(0)).ok()?;
    r.seek(SeekFrom::Start(0)).ok()?;
    mp4_walk(r, end, 0)
}

fn mp4_walk<R: Read + Seek>(r: &mut R, end: u64, depth: u32) -> Option<Colour> {
    if depth > 8 {
        return None;
    }
    while let Some((kind, body, box_end)) = read_box(r, end) {
        match &kind {
            b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" => {
                if let Some(c) = mp4_walk(r, box_end, depth + 1) {
                    return Some(c);
                }
            }
            b"stsd" => {
                r.seek(SeekFrom::Start(body + 8)).ok()?; // version/flags + entry count
                while let Some((entry, e_body, e_end)) = read_box(r, box_end) {
                    if VIDEO_ENTRIES.contains(&&entry) {
                        let mut c = Colour {
                            dolby_vision: matches!(&entry, b"dvh1" | b"dvhe" | b"dvav" | b"dva1" | b"dav1"),
                            ..Default::default()
                        };
                        // VisualSampleEntry fields are 78 bytes; child boxes follow.
                        r.seek(SeekFrom::Start(e_body + 78)).ok()?;
                        while let Some((child, c_body, c_end)) = read_box(r, e_end) {
                            match &child {
                                b"colr" => {
                                    let mut b = [0u8; 10];
                                    if r.read_exact(&mut b).is_ok() && &b[..4] == b"nclx" {
                                        c.transfer = Some(u16::from_be_bytes([b[6], b[7]]) as u64);
                                    }
                                }
                                b"dvcC" | b"dvvC" | b"dvwC" => c.dolby_vision = true,
                                b"mdcv" | b"SmDm" => c.mastering = true,
                                _ => {}
                            }
                            let _ = c_body;
                            r.seek(SeekFrom::Start(c_end)).ok()?;
                        }
                        return Some(c);
                    }
                    r.seek(SeekFrom::Start(e_end)).ok()?;
                }
            }
            _ => {}
        }
        r.seek(SeekFrom::Start(box_end)).ok()?;
        if box_end >= end {
            break;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // ---- tiny MKV builder
    fn el(id: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        let idb = id.to_be_bytes();
        let skip = idb.iter().position(|&b| b != 0).unwrap();
        v.extend_from_slice(&idb[skip..]);
        v.push(0x01); // 8-byte size
        v.extend_from_slice(&(payload.len() as u64).to_be_bytes()[1..]);
        v.extend_from_slice(payload);
        v
    }
    fn uint(id: u32, x: u64) -> Vec<u8> {
        el(id, &x.to_be_bytes())
    }
    fn mkv_file(colour: Option<Vec<u8>>, extra: Vec<u8>) -> Vec<u8> {
        let mut video = Vec::new();
        if let Some(c) = colour {
            video.extend(el(COLOUR, &c));
        }
        let mut entry = uint(TRACK_TYPE, 1);
        entry.extend(el(VIDEO, &video));
        entry.extend(extra);
        let audio = uint(TRACK_TYPE, 2);
        let mut tracks = el(TRACK_ENTRY, &audio);
        tracks.extend(el(TRACK_ENTRY, &entry));
        let mut f = el(EBML, &[]);
        // Segment with unknown size, like a live-muxed file.
        f.extend_from_slice(&[0x18, 0x53, 0x80, 0x67, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        f.extend(el(0x1549A966, &[])); // Info
        f.extend(el(TRACKS, &tracks));
        f
    }
    fn mkv_probe(bytes: Vec<u8>) -> Probe {
        mkv(&mut Cursor::new(bytes)).map(|c| c.verdict()).unwrap_or(Probe::Unknown)
    }

    #[test]
    fn mkv_pq_hlg_sdr_dv() {
        assert_eq!(mkv_probe(mkv_file(Some(uint(TRANSFER, 16)), vec![])), Probe::Hdr(HdrKind::Hdr10));
        assert_eq!(mkv_probe(mkv_file(Some(uint(TRANSFER, 18)), vec![])), Probe::Hdr(HdrKind::Hlg));
        assert_eq!(mkv_probe(mkv_file(Some(uint(TRANSFER, 1)), vec![])), Probe::Sdr);
        assert_eq!(mkv_probe(mkv_file(None, vec![])), Probe::Unknown);
        let dv = el(BLOCK_ADDITION_MAPPING, &uint(BLOCK_ADD_ID_TYPE, u32::from_be_bytes(*b"dvcC") as u64));
        assert_eq!(mkv_probe(mkv_file(Some(uint(TRANSFER, 16)), dv)), Probe::Hdr(HdrKind::DolbyVision));
        let mastering = el(MASTERING, &[]);
        assert_eq!(mkv_probe(mkv_file(Some(mastering), vec![])), Probe::Hdr(HdrKind::Hdr10));
    }

    // ---- tiny MP4 builder
    fn bx(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(payload);
        v
    }
    fn mp4_file(entry: &[u8; 4], transfer: Option<u16>, dv: bool, moov_last: bool) -> Vec<u8> {
        let mut se = vec![0u8; 78];
        if let Some(t) = transfer {
            let mut colr = b"nclx".to_vec();
            colr.extend_from_slice(&9u16.to_be_bytes());
            colr.extend_from_slice(&t.to_be_bytes());
            colr.extend_from_slice(&9u16.to_be_bytes());
            colr.push(0);
            se.extend(bx(b"colr", &colr));
        }
        if dv {
            se.extend(bx(b"dvcC", &[0u8; 24]));
        }
        let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
        stsd.extend(bx(entry, &se));
        let moov = bx(b"moov", &bx(b"trak", &bx(b"mdia", &bx(b"minf", &bx(b"stbl", &bx(b"stsd", &stsd))))));
        let mut f = bx(b"ftyp", b"isom\0\0\0\0");
        let mdat = bx(b"mdat", &[0u8; 64]);
        if moov_last {
            f.extend(mdat);
            f.extend(moov);
        } else {
            f.extend(moov);
            f.extend(mdat);
        }
        f
    }
    fn mp4_probe(bytes: Vec<u8>) -> Probe {
        mp4(&mut Cursor::new(bytes)).map(|c| c.verdict()).unwrap_or(Probe::Unknown)
    }

    #[test]
    fn mp4_pq_hlg_sdr_dv() {
        assert_eq!(mp4_probe(mp4_file(b"hvc1", Some(16), false, false)), Probe::Hdr(HdrKind::Hdr10));
        assert_eq!(mp4_probe(mp4_file(b"hvc1", Some(18), false, true)), Probe::Hdr(HdrKind::Hlg));
        assert_eq!(mp4_probe(mp4_file(b"avc1", Some(1), false, false)), Probe::Sdr);
        assert_eq!(mp4_probe(mp4_file(b"avc1", None, false, false)), Probe::Unknown);
        assert_eq!(mp4_probe(mp4_file(b"dvh1", Some(16), false, false)), Probe::Hdr(HdrKind::DolbyVision));
        assert_eq!(mp4_probe(mp4_file(b"hvc1", Some(16), true, true)), Probe::Hdr(HdrKind::DolbyVision));
    }

    #[test]
    fn names() {
        assert!(name_says_hdr(Path::new(r"C:\a\Show.S01E01.2160p.HDR.mkv")));
        assert!(name_says_hdr(Path::new(r"C:\a\Movie.2160p.DV.HEVC.mkv")));
        assert!(name_says_hdr(Path::new(r"C:\a\[Group] Anime - 01 [HDR10].mkv")));
        assert!(!name_says_hdr(Path::new(r"C:\a\[SubsPlease] Anime - 01 (1080p).mkv")));
        assert!(!name_says_hdr(Path::new(r"C:\a\Shadrach.mkv")));
    }
}

#[cfg(test)]
mod real_files {
    use super::*;

    /// Read-only check on real files: `KAISYNC_PROBE_FILES="a.mkv|b.mp4" cargo test real_probe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn real_probe() {
        let list = std::env::var("KAISYNC_PROBE_FILES").unwrap_or_default();
        for p in list.split('|').filter(|s| !s.is_empty()) {
            let path = Path::new(p);
            let started = std::time::Instant::now();
            let meta = File::open(path).ok().and_then(|f| {
                let mut r = BufReader::new(f);
                let mut m = [0u8; 8];
                r.read_exact(&mut m).ok()?;
                r.seek(SeekFrom::Start(0)).ok()?;
                if m[..4] == [0x1A, 0x45, 0xDF, 0xA3] { mkv(&mut r) } else { mp4(&mut r) }
            });
            println!(
                "PROBE meta={:?} verdict={:?} final={:?} [{} ms] {}",
                meta,
                meta.as_ref().map(|c| c.verdict()),
                probe(path),
                started.elapsed().as_millis(),
                path.file_name().unwrap().to_string_lossy()
            );
        }
    }
}
