mod commands;
mod desktop;
mod notifications;

use std::{fs::DirBuilder, os::unix::fs::DirBuilderExt};

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::ShortcutState;
use worklog_core::Store;

use commands::*;
use desktop::{Desktop, LaunchMode};

fn main() {
    let mode = LaunchMode::parse(std::env::args().skip(1));
    let mut builder = tauri::Builder::default()
        // The first plugin must intercept launcher actions before any windows are opened.
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            let handle = app.clone();
            let mode = LaunchMode::parse(args.into_iter().skip(1));
            if let Err(error) = app.run_on_main_thread(move || desktop::launch(&handle, mode, true))
            {
                eprintln!("WorkLog could not handle the launcher action: {error}");
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .plugin(tauri_plugin_dialog::init());

    if desktop::x11_shortcuts() {
        builder = builder.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state == ShortcutState::Pressed {
                        desktop::show_capture(app);
                    }
                })
                .build(),
        );
    }

    let app = builder
        .setup(move |app| {
            let directory = app.path().app_data_dir()?;
            let path = directory.join("worklog.sqlite3");
            let store = DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&directory)
                .map_err(worklog_core::Error::from)
                .and_then(|_| Store::open(&path));
            let store = match store {
                Ok(store) => store,
                Err(error) => {
                    let message = format!("WorkLog could not open {}.\n\n{error}", path.display());
                    eprintln!("{message}");
                    let handle = app.handle().clone();
                    app.dialog()
                        .message(message)
                        .title("WorkLog")
                        .kind(MessageDialogKind::Error)
                        .show(move |_| handle.exit(1));
                    return Ok(());
                }
            };
            let settings = store.settings().unwrap_or_default();
            let desktop = Desktop::new(path, store, &settings);
            app.manage(desktop);
            desktop::apply_theme(app.handle(), settings.theme);
            desktop::apply_shortcut(app.handle(), &settings.shortcut);
            let tray_available = match desktop::create_tray(app.handle()) {
                Ok(()) => true,
                Err(error) => {
                    eprintln!("WorkLog could not create the tray icon: {error}");
                    false
                }
            };
            notifications::start(app.handle().clone());
            desktop::launch(app.handle(), mode, tray_available);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.label() == "dashboard"
                    && window
                        .try_state::<Desktop>()
                        .is_some_and(|desktop| !desktop.minimize_to_tray())
                {
                    let _ = window.minimize();
                } else {
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            capture_context,
            hide_capture,
            open_capture,
            take_dashboard_route,
            capture_entry,
            track_work,
            workday_summary,
            update_work_session,
            delete_work_session,
            list_entries,
            save_entry,
            delete_entry,
            day_review,
            month_reviews,
            save_review,
            save_category,
            reorder_categories,
            save_preferences,
            set_launch_at_login,
            transfer_data,
            test_notification,
            quit_worklog
        ])
        .build(tauri::generate_context!())
        .expect("WorkLog could not start");

    app.run(|_, event| {
        if let RunEvent::ExitRequested {
            api, code: None, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}
