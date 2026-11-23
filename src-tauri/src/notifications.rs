use std::time::Duration;

use chrono::{Local, Utc};
use notify_rust::{Hint, Notification, NotificationResponse};
use tauri::{AppHandle, Manager};
use worklog_core::{Reminder, TaskCheckAction, TaskCheckReminder, TaskReminder, TrackingAction};

use crate::desktop::{self, CaptureMode, DashboardRoute, Desktop};

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            let result = app.state::<Desktop>().db().and_then(|mut db| {
                db.claim_reminder(Local::now())
                    .map_err(|error| error.to_string())
            });
            match result {
                Ok(Some(reminder)) => {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = deliver(handle, reminder).await {
                            eprintln!("WorkLog could not deliver the daily reminder: {error}");
                        }
                    });
                }
                Ok(None) => {}
                Err(error) => eprintln!("WorkLog could not check reminders: {error}"),
            }
            let result = app.state::<Desktop>().db().and_then(|mut db| {
                db.claim_task_reminder(Utc::now().timestamp_millis())
                    .map_err(|error| error.to_string())
            });
            match result {
                Ok(Some(reminder)) => {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(error) = deliver_task_reminder(&handle, &reminder).await {
                            eprintln!("WorkLog could not deliver the task reminder: {error}");
                            let _ = handle.state::<Desktop>().db().and_then(|mut db| {
                                db.retry_task_reminder(&reminder)
                                    .map_err(|error| error.to_string())
                            });
                        }
                    });
                }
                Ok(None) => {}
                Err(error) => eprintln!("WorkLog could not check task reminders: {error}"),
            }
            let result = app.state::<Desktop>().db().and_then(|mut db| {
                db.claim_task_checks(Utc::now().timestamp_millis())
                    .map_err(|error| error.to_string())
            });
            match result {
                Ok(reminders) => {
                    for reminder in reminders {
                        let handle = app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(error) = deliver_task_check(&handle, &reminder).await {
                                eprintln!(
                                    "WorkLog could not deliver the hourly task check: {error}"
                                );
                                let _ = handle.state::<Desktop>().db().and_then(|mut db| {
                                    db.retry_task_check(&reminder)
                                        .map_err(|error| error.to_string())
                                });
                            }
                        });
                    }
                }
                Err(error) => eprintln!("WorkLog could not check running tasks: {error}"),
            }
            // Re-evaluate civil time after suspension or timezone changes.
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
}

async fn deliver_task_check(app: &AppHandle, reminder: &TaskCheckReminder) -> Result<(), String> {
    let description = reminder
        .task
        .description
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let mut builder = Notification::new();
    builder.appname("WorkLog")
        .summary("Still working on this task?")
        .body(&format!("{description}\nThis timer has been running for at least an hour. Switch task stops it and opens Quick Capture for your next task."))
        .icon("worklog")
        .hint(Hint::DesktopEntry("WorkLog".into()))
        .hint(Hint::SuppressSound(!reminder.sound))
        .action("default", "Open WorkLog")
        .action("keep-working", "Keep working")
        .action("switch-task", "Switch task");
    if reminder.sound {
        builder.hint(Hint::SoundName("message-new-instant".into()));
    }
    let notification = builder
        .show_async()
        .await
        .map_err(|error| error.to_string())?;
    let waiting = notification.wait_for_action_async(|response| {
        let action = match response {
            NotificationResponse::Default => {
                desktop::show_capture(app);
                return;
            }
            NotificationResponse::Action(name) if name == "keep-working" => {
                TaskCheckAction::KeepWorking
            }
            NotificationResponse::Action(name) if name == "switch-task" => {
                TaskCheckAction::SwitchTask
            }
            _ => return,
        };
        let result = app.state::<Desktop>().db().and_then(|mut db| {
            db.answer_task_check(reminder, action, Utc::now().timestamp_millis())
                .map_err(|error| error.to_string())
        });
        match result {
            Ok(true) if matches!(action, TaskCheckAction::SwitchTask) => {
                desktop::changed(app);
                desktop::show_capture_mode(app, CaptureMode::Task);
            }
            Ok(_) => {}
            Err(error) => eprintln!("WorkLog could not handle the hourly task check: {error}"),
        }
    });
    let _ = tokio::time::timeout(Duration::from_secs(60 * 60), waiting).await;
    notification.close_async().await;
    Ok(())
}

async fn deliver_task_reminder(app: &AppHandle, reminder: &TaskReminder) -> Result<(), String> {
    let mut builder = Notification::new();
    builder
        .appname("WorkLog")
        .summary("What are you working on?")
        .body("Your workday is open and no task is running. Start a task or take a break.")
        .icon("worklog")
        .hint(Hint::DesktopEntry("WorkLog".into()))
        .hint(Hint::SuppressSound(!reminder.sound))
        .action("default", "Start task")
        .action("start-task", "Start task")
        .action("take-break", "Take break");
    if reminder.sound {
        builder.hint(Hint::SoundName("message-new-instant".into()));
    }
    let notification = builder
        .show_async()
        .await
        .map_err(|error| error.to_string())?;
    let waiting = notification.wait_for_action_async(|response| {
        let action = match response {
            NotificationResponse::Default => "start-task",
            NotificationResponse::Action(name) => name.as_str(),
            _ => return,
        };
        let result = (|| {
            let desktop = app.state::<Desktop>();
            let mut db = desktop.db()?;
            if !db
                .task_reminder_is_current(reminder, Utc::now().timestamp_millis())
                .map_err(|error| error.to_string())?
            {
                return Ok(());
            }
            match action {
                "take-break" => {
                    db.track(TrackingAction::StartBreak)
                        .map_err(|error| error.to_string())?;
                    drop(db);
                    desktop::changed(app);
                }
                "start-task" => {
                    drop(db);
                    desktop::show_capture_mode(app, CaptureMode::Task);
                }
                _ => {}
            }
            Ok::<(), String>(())
        })();
        if let Err(error) = result {
            eprintln!("WorkLog could not handle the task reminder: {error}");
        }
    });
    let _ = tokio::time::timeout(Duration::from_secs(5 * 60), waiting).await;
    notification.close_async().await;
    Ok(())
}

async fn deliver(app: AppHandle, reminder: Reminder) -> Result<(), String> {
    let body = match reminder.entry_count {
        0 => "Add any missing activities and review your day.".into(),
        1 => "You have 1 activity today. Review your work before finishing.".into(),
        count => format!("You have {count} activities today. Review your work before finishing."),
    };
    let notification = base("WorkLog — Daily Review", &body, reminder.sound)
        .action("snooze", &format!("Snooze {} min", reminder.snooze_minutes))
        .show_async()
        .await
        .map_err(|error| error.to_string())?;
    let date = reminder.date;
    let waiting = notification.wait_for_action_async(move |response| match response {
        NotificationResponse::Default => open_review(&app, date),
        NotificationResponse::Action(name) if name == "review" => open_review(&app, date),
        NotificationResponse::Action(name) if name == "snooze" => {
            let result = app.state::<Desktop>().db().and_then(|mut db| {
                db.snooze_reminder(&date, Local::now())
                    .map_err(|error| error.to_string())
            });
            if let Err(error) = result {
                eprintln!("WorkLog could not snooze the reminder: {error}");
            }
        }
        _ => {}
    });
    let _ = tokio::time::timeout(Duration::from_secs(3 * 60 * 60), waiting).await;
    notification.close_async().await;
    Ok(())
}

pub async fn test(app: AppHandle, sound: bool) -> Result<(), String> {
    let notification = base(
        "WorkLog",
        "Daily review notifications will appear here.",
        sound,
    )
    .show_async()
    .await
    .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn(async move {
        let waiting = notification.wait_for_action_async(move |response| {
            if matches!(
                response,
                NotificationResponse::Default | NotificationResponse::Action(_)
            ) {
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    desktop::show_dashboard(&handle, DashboardRoute::Journal)
                });
            }
        });
        let _ = tokio::time::timeout(Duration::from_secs(60), waiting).await;
        notification.close_async().await;
    });
    Ok(())
}

fn base(title: &str, body: &str, sound: bool) -> Notification {
    let mut notification = Notification::new();
    notification
        .appname("WorkLog")
        .summary(title)
        .body(body)
        .icon("worklog")
        .hint(Hint::DesktopEntry("WorkLog".into()))
        .hint(Hint::SuppressSound(!sound))
        .action("default", "Open WorkLog")
        .action("review", "Open Dashboard");
    if sound {
        notification.hint(Hint::SoundName("message-new-instant".into()));
    }
    notification
}

fn open_review(app: &AppHandle, date: String) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        desktop::show_dashboard(&handle, DashboardRoute::Review { date })
    });
}
