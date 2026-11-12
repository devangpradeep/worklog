use std::time::Duration;

use chrono::Local;
use notify_rust::{Hint, Notification, NotificationResponse};
use tauri::{AppHandle, Manager};
use worklog_core::Reminder;

use crate::desktop::{self, DashboardRoute, Desktop};

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
            // Re-evaluate civil time after suspension or timezone changes.
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
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
