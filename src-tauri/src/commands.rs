use chrono::{Local, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use worklog_core::{
    Category, CategoryInput, Entry, EntryInput, EntryQuery, Review, ReviewAction, Settings,
    TrackingAction, TrackingState, WorkSession, WorkdaySummary,
};

use crate::{
    desktop::{self, changed, CaptureContext, CaptureMode, DashboardRoute, Desktop},
    notifications,
};

type Reply<T> = Result<T, String>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    settings: Settings,
    categories: Vec<Category>,
    capture_category: Option<i64>,
    database_path: String,
    launch_at_login: Option<bool>,
    x11_shortcuts: bool,
    shortcut_error: Option<String>,
    tracking: TrackingState,
}

#[tauri::command]
pub async fn snapshot(app: AppHandle, desktop: State<'_, Desktop>) -> Reply<Snapshot> {
    let (settings, categories, capture_category, tracking) = {
        let store = desktop.db()?;
        let settings = store.settings().map_err(message)?;
        let categories = store.categories().map_err(message)?;
        let capture_category = store.capture_category(&settings).map_err(message)?;
        (
            settings,
            categories,
            capture_category,
            store.tracking_state().map_err(message)?,
        )
    };
    Ok(Snapshot {
        settings,
        categories,
        capture_category,
        database_path: desktop.path.display().to_string(),
        launch_at_login: app.autolaunch().is_enabled().ok(),
        x11_shortcuts: desktop::x11_shortcuts(),
        shortcut_error: desktop.shortcut_error(),
        tracking,
    })
}

#[tauri::command]
pub fn capture_context(desktop: State<'_, Desktop>) -> Reply<CaptureContext> {
    desktop
        .capture
        .lock()
        .map(|context| *context)
        .map_err(message)
}

#[tauri::command]
pub fn hide_capture(app: AppHandle, desktop: State<'_, Desktop>, generation: u64) -> Reply<()> {
    if capture_context(desktop)?.generation == generation {
        if let Some(window) = app.get_webview_window("capture") {
            window.hide().map_err(message)?;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn open_capture(app: AppHandle, mode: Option<CaptureMode>) {
    desktop::show_capture_mode(&app, mode.unwrap_or_default());
}

#[tauri::command]
pub fn take_dashboard_route(desktop: State<'_, Desktop>) -> Option<DashboardRoute> {
    desktop.route.lock().ok()?.take()
}

#[tauri::command]
pub async fn capture_entry(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    description: String,
    category_id: Option<i64>,
    capture_key: String,
    mode: Option<CaptureMode>,
) -> Reply<Entry> {
    let entry = {
        let mut db = desktop.db()?;
        let input = EntryInput {
            description,
            category_id,
            activity_at: None,
            end_at: None,
            update_end: false,
            kind: Default::default(),
            reference: None,
            auto_reference: true,
        };
        match mode.unwrap_or_default() {
            CaptureMode::Quick => db.save_entry(None, input, Some(&capture_key)),
            CaptureMode::Task => db.start_task(input, &capture_key),
        }
        .map_err(message)?
    };
    changed(&app);
    Ok(entry)
}

#[tauri::command]
pub async fn track_work(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    action: TrackingAction,
) -> Reply<()> {
    desktop.db()?.track(action).map_err(message)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn workday_summary(desktop: State<'_, Desktop>, date: String) -> Reply<WorkdaySummary> {
    desktop
        .db()?
        .workday_summary(&date, Utc::now().timestamp_millis())
        .map_err(message)
}

#[tauri::command]
pub async fn update_work_session(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    session: WorkSession,
    started_at: i64,
    ended_at: Option<i64>,
) -> Reply<()> {
    desktop
        .db()?
        .update_work_session(session, started_at, ended_at)
        .map_err(message)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn delete_work_session(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    session: WorkSession,
) -> Reply<()> {
    desktop
        .db()?
        .delete_work_session(session)
        .map_err(message)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn list_entries(desktop: State<'_, Desktop>, query: EntryQuery) -> Reply<Vec<Entry>> {
    desktop.db()?.entries(query).map_err(message)
}

#[tauri::command]
pub async fn save_entry(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    id: Option<i64>,
    input: EntryInput,
) -> Reply<Entry> {
    let entry = desktop.db()?.save_entry(id, input, None).map_err(message)?;
    changed(&app);
    Ok(entry)
}

#[tauri::command]
pub async fn delete_entry(app: AppHandle, desktop: State<'_, Desktop>, id: i64) -> Reply<()> {
    desktop.db()?.delete_entry(id).map_err(message)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn day_review(desktop: State<'_, Desktop>, date: String) -> Reply<Review> {
    desktop.db()?.review(&date).map_err(message)
}

#[tauri::command]
pub async fn month_reviews(desktop: State<'_, Desktop>, month: String) -> Reply<Vec<Review>> {
    desktop.db()?.reviews(&month).map_err(message)
}

#[tauri::command]
pub async fn save_review(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    date: String,
    notes: String,
    action: ReviewAction,
) -> Reply<Review> {
    let review = desktop
        .db()?
        .save_review(&date, &notes, action)
        .map_err(message)?;
    changed(&app);
    Ok(review)
}

#[tauri::command]
pub async fn save_category(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    input: CategoryInput,
) -> Reply<()> {
    desktop.db()?.save_category(input).map_err(message)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn reorder_categories(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    ids: Vec<i64>,
) -> Reply<()> {
    desktop.db()?.reorder_categories(ids).map_err(message)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn save_preferences(
    app: AppHandle,
    desktop: State<'_, Desktop>,
    settings: Settings,
) -> Reply<Settings> {
    let saved = {
        let mut db = desktop.db()?;
        let previous = db.settings().map_err(message)?;
        let shortcut_changed = settings.shortcut.trim() != previous.shortcut;
        if shortcut_changed {
            desktop::register_shortcut(&app, settings.shortcut.trim())?;
        }
        match db.save_settings(settings) {
            Ok(saved) => saved,
            Err(error) => {
                if shortcut_changed {
                    desktop::apply_shortcut(&app, &previous.shortcut);
                }
                return Err(message(error));
            }
        }
    };
    desktop.update_preferences(&app, &saved);
    changed(&app);
    Ok(saved)
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> Reply<()> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(message)?;
    changed(&app);
    Ok(())
}

#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Transfer {
    Backup,
    Restore,
    Export,
}

#[tauri::command]
pub async fn transfer_data(
    app: AppHandle,
    window: WebviewWindow,
    action: Transfer,
) -> Reply<Option<String>> {
    if window.label() != "dashboard" {
        return Err("Open Settings in the dashboard to manage backups.".into());
    }
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let file = worker_app.dialog().file().set_parent(&window);
        let date = Local::now().format("%Y-%m-%d");
        let chosen = match action {
            Transfer::Backup => file.set_title("Back up WorkLog")
                .set_file_name(format!("worklog-{date}.sqlite3"))
                .add_filter("WorkLog backup", &["sqlite3"]).blocking_save_file(),
            Transfer::Export => file.set_title("Export work entries")
                .set_file_name(format!("worklog-entries-{date}.csv"))
                .add_filter("CSV", &["csv"]).blocking_save_file(),
            Transfer::Restore => file.set_title("Choose a WorkLog backup")
                .add_filter("WorkLog backup", &["sqlite3", "db", "sqlite"]).blocking_pick_file(),
        };
        let Some(path) = chosen else { return Ok(None) };
        let path = path.into_path().map_err(message)?;
        if matches!(action, Transfer::Restore) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let accepted = worker_app.dialog()
                .message(format!("Replace the current journal with “{name}”? This replaces your activities, work sessions, review notes, categories, and database preferences."))
                .parent(&window)
                .title("Restore WorkLog backup")
                .kind(MessageDialogKind::Warning)
                .buttons(MessageDialogButtons::OkCancelCustom("Restore".into(), "Cancel".into()))
                .blocking_show();
            if !accepted {
                return Ok(None);
            }
        }
        let desktop = worker_app.state::<Desktop>();
        let mut db = desktop.db()?;
        match action {
            Transfer::Backup => db.backup(&path),
            Transfer::Restore => db.restore(&path),
            Transfer::Export => db.export_csv(&path),
        }.map_err(message)?;
        Ok::<_, String>(Some(path.display().to_string()))
    }).await.map_err(message)??;
    if result.is_some() && matches!(action, Transfer::Restore) {
        let settings = app.state::<Desktop>().db()?.settings().map_err(message)?;
        let handle = app.clone();
        app.run_on_main_thread(move || {
            handle
                .state::<Desktop>()
                .update_preferences(&handle, &settings);
            desktop::apply_shortcut(&handle, &settings.shortcut);
            let _ = handle.emit("data-restored", ());
            changed(&handle);
        })
        .map_err(message)?;
    }
    Ok(result)
}

#[tauri::command]
pub async fn test_notification(app: AppHandle, desktop: State<'_, Desktop>) -> Reply<()> {
    let sound = desktop
        .db()?
        .settings()
        .map_err(message)?
        .notification_sound;
    notifications::test(app, sound).await
}

#[tauri::command]
pub fn quit_worklog(app: AppHandle) {
    app.exit(0);
}

fn message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
