use std::{
    path::PathBuf,
    str::FromStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, MutexGuard,
    },
};

use gtk::prelude::{GtkWindowExt, WidgetExt};
use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewWindow,
};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use worklog_core::{EntryKind, Settings, Store, Theme, TrackingAction};

pub struct Desktop {
    pub path: PathBuf,
    pub store: Mutex<Store>,
    pub capture: Mutex<CaptureContext>,
    pub route: Mutex<Option<DashboardRoute>>,
    minimize_to_tray: AtomicBool,
    shortcut: Mutex<ShortcutRegistration>,
}

#[derive(Default)]
struct ShortcutRegistration {
    shortcut: Option<Shortcut>,
    error: Option<String>,
}

impl Desktop {
    pub fn new(path: PathBuf, store: Store, settings: &Settings) -> Self {
        Self {
            path,
            store: Mutex::new(store),
            route: Mutex::new(None),
            capture: Mutex::default(),
            minimize_to_tray: AtomicBool::new(settings.minimize_to_tray),
            shortcut: Mutex::default(),
        }
    }

    pub fn db(&self) -> Result<MutexGuard<'_, Store>, String> {
        self.store
            .lock()
            .map_err(|_| "The database is unavailable. Restart WorkLog.".into())
    }

    pub fn minimize_to_tray(&self) -> bool {
        self.minimize_to_tray.load(Ordering::Relaxed)
    }

    pub fn update_preferences(&self, app: &AppHandle, settings: &Settings) {
        self.minimize_to_tray
            .store(settings.minimize_to_tray, Ordering::Relaxed);
        apply_theme(app, settings.theme);
    }

    pub fn shortcut_error(&self) -> Option<String> {
        self.shortcut
            .lock()
            .ok()
            .and_then(|registration| registration.error.clone())
    }
}

#[derive(Clone, Copy)]
pub enum LaunchMode {
    Journal,
    Capture,
    Settings,
    Background,
}

impl LaunchMode {
    pub fn parse(args: impl Iterator<Item = String>) -> Self {
        for arg in args {
            match arg.as_str() {
                "--quick-add" => return Self::Capture,
                "--settings" => return Self::Settings,
                "--background" => return Self::Background,
                _ => {}
            }
        }
        Self::Journal
    }
}

#[derive(Clone, Serialize)]
#[serde(tag = "page", rename_all = "camelCase")]
pub enum DashboardRoute {
    Journal,
    Settings,
    Review { date: String },
}

pub fn launch(app: &AppHandle, mode: LaunchMode, tray_available: bool) {
    if app.try_state::<Desktop>().is_none() {
        return;
    }
    match mode {
        LaunchMode::Capture => show_capture(app),
        LaunchMode::Settings => show_dashboard(app, DashboardRoute::Settings),
        LaunchMode::Background if tray_available => {}
        LaunchMode::Background | LaunchMode::Journal => {
            show_dashboard(app, DashboardRoute::Journal)
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    #[default]
    Quick,
    Task,
}

#[derive(Default, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureContext {
    pub generation: u64,
    pub mode: CaptureMode,
}

pub fn show_capture(app: &AppHandle) {
    show_capture_mode(app, CaptureMode::Quick);
}

pub fn show_capture_mode(app: &AppHandle, mode: CaptureMode) {
    if let (Some(window), Some(desktop)) = (
        app.get_webview_window("capture"),
        app.try_state::<Desktop>(),
    ) {
        let Ok(mut capture) = desktop.capture.lock() else {
            return;
        };
        capture.generation += 1;
        capture.mode = mode;
        let context = *capture;
        drop(capture);
        let title = match mode {
            CaptureMode::Quick => "Quick Capture · WorkLog",
            CaptureMode::Task => "Start task · WorkLog",
        };
        let _ = window.set_title(title);
        reveal(&window);
        let _ = window.emit("capture-opened", context);
    }
}

pub fn show_dashboard(app: &AppHandle, route: DashboardRoute) {
    if let (Some(window), Some(desktop)) = (
        app.get_webview_window("dashboard"),
        app.try_state::<Desktop>(),
    ) {
        if let Ok(mut pending) = desktop.route.lock() {
            *pending = Some(route);
        }
        reveal(&window);
        let _ = window.emit("dashboard-route", ());
    }
}

fn reveal(window: &WebviewWindow) {
    let handle = window.clone();
    if let Err(error) = window.run_on_main_thread(move || match handle.gtk_window() {
        Ok(window) => {
            // Request focus after GTK maps the window. Tauri queues visibility separately on Linux.
            window.show_all();
            window.deiconify();
            window.present();
        }
        Err(error) => eprintln!("WorkLog could not activate {}: {error}", handle.label()),
    }) {
        eprintln!("WorkLog could not activate {}: {error}", window.label());
    }
}

pub fn apply_theme(app: &AppHandle, theme: Theme) {
    app.set_theme(match theme {
        Theme::System => None,
        Theme::Light => Some(tauri::Theme::Light),
        Theme::Dark => Some(tauri::Theme::Dark),
    });
}

pub fn x11_shortcuts() -> bool {
    match std::env::var("XDG_SESSION_TYPE").as_deref() {
        Ok("wayland") => false,
        Ok("x11") => true,
        _ => std::env::var_os("WAYLAND_DISPLAY").is_none() && std::env::var_os("DISPLAY").is_some(),
    }
}

pub fn apply_shortcut(app: &AppHandle, value: &str) {
    if let Err(error) = register_shortcut(app, value) {
        eprintln!("WorkLog could not apply the saved shortcut: {error}");
        let _ = register_shortcut(app, "");
        let desktop = app.state::<Desktop>();
        if let Ok(mut registration) = desktop.shortcut.lock() {
            registration.error = Some(error);
        };
    }
}

pub fn register_shortcut(app: &AppHandle, value: &str) -> Result<(), String> {
    if !x11_shortcuts() {
        return Ok(());
    }
    validate_shortcut(value)?;
    let desktop = app.state::<Desktop>();
    let mut registration = desktop
        .shortcut
        .lock()
        .map_err(|_| "The global shortcut is unavailable. Restart WorkLog.".to_string())?;
    change_shortcut(app, &mut registration.shortcut, value)?;
    registration.error = None;
    Ok(())
}

fn change_shortcut(
    app: &AppHandle,
    current: &mut Option<Shortcut>,
    value: &str,
) -> Result<(), String> {
    let next = if value.is_empty() {
        None
    } else {
        Some(
            Shortcut::from_str(value)
                .map_err(|_| "Use a shortcut such as Ctrl+Shift+Space.".to_string())?,
        )
    };
    if next == *current {
        return Ok(());
    }
    let manager = app.global_shortcut();
    if let Some(next) = next {
        manager
            .register(next)
            .map_err(|error| format!("Could not register {value}: {error}"))?;
    }
    if let Some(old) = *current {
        if let Err(error) = manager.unregister(old) {
            if let Some(next) = next {
                let _ = manager.unregister(next);
            }
            return Err(format!("Could not release the previous shortcut: {error}"));
        }
    }
    *current = next;
    Ok(())
}

fn validate_shortcut(value: &str) -> Result<(), String> {
    if x11_shortcuts() && !value.is_empty() {
        let shortcut = Shortcut::from_str(value)
            .map_err(|_| "Use a shortcut such as Ctrl+Shift+Space.".to_string())?;
        if shortcut.mods.is_empty() {
            return Err("Include a modifier such as Ctrl, Alt, Shift, or Super.".into());
        }
    }
    Ok(())
}

struct TrayTracking {
    day: MenuItem<tauri::Wry>,
    stop_task: MenuItem<tauri::Wry>,
    pause: MenuItem<tauri::Wry>,
}

pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    let day = MenuItem::with_id(app, "workday", "Start day", true, None::<&str>)?;
    let task = MenuItem::with_id(app, "start_task", "Start task…", true, None::<&str>)?;
    let stop_task = MenuItem::with_id(app, "stop_task", "Stop task", false, None::<&str>)?;
    let pause = MenuItem::with_id(app, "break", "Take break", true, None::<&str>)?;
    let tracking_divider = PredefinedMenuItem::separator(app)?;
    let quick = MenuItem::with_id(app, "quick", "Quick Add", true, None::<&str>)?;
    let journal = MenuItem::with_id(app, "journal", "Open Dashboard", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let divider = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit WorkLog", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &day,
            &task,
            &stop_task,
            &pause,
            &tracking_divider,
            &quick,
            &journal,
            &settings,
            &divider,
            &quit,
        ],
    )?;
    let mut tray = TrayIconBuilder::with_id("worklog").tooltip("WorkLog");
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quick" => show_capture(app),
            "start_task" => show_capture_mode(app, CaptureMode::Task),
            id @ ("workday" | "break" | "stop_task") => track_from_tray(app, id),
            "journal" => show_dashboard(app, DashboardRoute::Journal),
            "settings" => show_dashboard(app, DashboardRoute::Settings),
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    app.manage(TrayTracking {
        day,
        stop_task,
        pause,
    });
    refresh_tracking_menu(app);
    Ok(())
}

fn track_from_tray(app: &AppHandle, id: &str) {
    let result = (|| {
        let desktop = app.state::<Desktop>();
        let mut db = desktop.db()?;
        let state = db.tracking_state().map_err(|error| error.to_string())?;
        let action =
            match id {
                "workday" => state.session.map_or(TrackingAction::StartDay, |session| {
                    TrackingAction::EndDay { id: session.id }
                }),
                "break" => state
                    .entry
                    .filter(|entry| entry.kind == EntryKind::Break)
                    .map_or(TrackingAction::StartBreak, |entry| {
                        TrackingAction::EndBreak { id: entry.id }
                    }),
                _ => match state
                    .entry
                    .filter(|entry| entry.kind == EntryKind::Activity)
                {
                    Some(entry) => TrackingAction::StopTask { id: entry.id },
                    None => return Ok(()),
                },
            };
        db.track(action).map_err(|error| error.to_string())
    })();
    if let Err(error) = result {
        app.dialog()
            .message(error)
            .title("WorkLog")
            .kind(MessageDialogKind::Error)
            .show(|_| {});
    } else {
        changed(app);
    }
}

pub fn changed(app: &AppHandle) {
    let _ = app.emit("data-changed", ());
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || refresh_tracking_menu(&handle));
}

fn refresh_tracking_menu(app: &AppHandle) {
    let Some(menu) = app.try_state::<TrayTracking>() else {
        return;
    };
    let desktop = app.state::<Desktop>();
    let Ok(state) = desktop
        .db()
        .and_then(|db| db.tracking_state().map_err(|error| error.to_string()))
    else {
        return;
    };
    let _ = menu.day.set_text(if state.session.is_some() {
        "End day"
    } else {
        "Start day"
    });
    let kind = state.entry.map(|entry| entry.kind);
    let _ = menu
        .stop_task
        .set_enabled(kind == Some(EntryKind::Activity));
    let _ = menu.pause.set_text(if kind == Some(EntryKind::Break) {
        "End break"
    } else {
        "Take break"
    });
}
