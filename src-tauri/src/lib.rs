mod commands;
mod detector;
mod engine;
mod hdr;
mod logging;
mod settings;
mod state_machine;
mod system;
mod tray;

use std::sync::Mutex;

use tauri::{AppHandle, Manager, WindowEvent};
use tracing_appender::non_blocking::WorkerGuard;

use crate::{engine::Engine, settings::Settings};

static LOG_GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

/// Normal Exit: apply Idle (restore HDR per the rule), flush logs, quit.
///
/// Runs off the main thread: the tray menu calls this on the main thread, and the engine
/// updates the tray icon (which needs the main thread) while applying Idle. Waiting for the
/// engine here would block both until the shutdown timeout.
pub fn exit_app(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if let Some(engine) = app.try_state::<Engine>() {
            engine.shutdown();
        }
        tracing::info!("KaiSync exiting");
        drop(LOG_GUARD.lock().unwrap().take());
        app.exit(0);
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    *LOG_GUARD.lock().unwrap() = logging::init();
    let background = std::env::args().any(|a| a == system::BACKGROUND_ARG);

    tauri::Builder::default()
        // Must be first: a second launch shows the existing window instead.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tracing::info!("second launch {:?}", args);
            if !args.iter().any(|a| a == system::BACKGROUND_ARG) {
                tray::show_main(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args([system::BACKGROUND_ARG])
                .app_name("KaiSync")
                .build(),
        )
        .setup(move |app| {
            let mut settings = Settings::load();
            // Start with Windows follows the setting (on by default).
            system::set_autostart(app.handle(), settings.autostart);
            // The desktop toggle shows what is really there (the installer may have made it,
            // or you may have deleted it).
            if !cfg!(debug_assertions) {
                let exists = system::shortcut_exists();
                if settings.desktop_shortcut != exists {
                    settings.desktop_shortcut = exists;
                    settings.save();
                }
            }
            let lang = settings.language.clone();
            let engine = Engine::start(app.handle().clone(), settings);
            app.manage(engine);
            tray::create(app.handle(), &lang)?;
            tray::create_menu_window(app.handle())?;
            let st = app.state::<Engine>().status.lock().unwrap().clone();
            tray::update(app.handle(), &st);
            if background {
                tracing::info!("started with --background: tray only");
            } else {
                tray::show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the window hides it to the tray.
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let _ = window.hide();
            }
            // The glass tray menu closes when you click elsewhere.
            WindowEvent::Focused(false) if window.label() == tray::MENU_LABEL => {
                let _ = window.hide();
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_library,
            commands::get_settings,
            commands::set_paused,
            commands::rescan,
            commands::set_source,
            commands::add_game_folder,
            commands::remove_game_folder,
            commands::add_manual_game,
            commands::remove_manual_game,
            commands::set_game_enabled,
            commands::pick_folder_exe,
            commands::set_language,
            commands::set_general,
            commands::set_player,
            commands::open_logs_folder,
            commands::list_displays,
            commands::set_hdr_display,
            commands::hdr_test,
            commands::tray_action,
        ])
        .run(tauri::generate_context!())
        .expect("error while running KaiSync");
}
