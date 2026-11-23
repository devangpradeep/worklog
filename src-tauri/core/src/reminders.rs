use chrono::{DateTime, Datelike, Local, NaiveTime, TimeDelta, TimeZone};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use crate::{
    day_bounds, store::read_review, Entry, EntryKind, Result, ReviewStatus, Settings, Store,
};

const TASK_REMINDER_INTERVAL: i64 = 5 * 60_000;
const TASK_CHECK_INTERVAL: i64 = 60 * 60_000;

#[derive(Debug)]
pub struct TaskCheckReminder {
    pub task: Entry,
    pub sent_at: i64,
    pub sound: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum TaskCheckAction {
    KeepWorking,
    SwitchTask,
}

#[derive(Deserialize, Serialize)]
struct TaskCheckDelivery {
    task_id: i64,
    started_at: i64,
    sent_at: i64,
}

#[derive(Debug)]
pub struct TaskReminder {
    pub session_id: i64,
    pub idle_since: i64,
    pub sent_at: i64,
    pub sound: bool,
}

#[derive(Deserialize, Serialize)]
struct TaskReminderDelivery {
    session_id: i64,
    idle_since: i64,
    sent_at: i64,
}

fn idle_workday(conn: &Connection, now: i64) -> Result<Option<(i64, i64)>> {
    Ok(conn
        .query_row(
            "SELECT id, MAX(started_at, COALESCE((SELECT MAX(end_at) FROM work_entries
          WHERE end_at <= ?1 AND end_at >= work_sessions.started_at), started_at))
         FROM work_sessions WHERE ended_at IS NULL
           AND NOT EXISTS(SELECT 1 FROM work_entries WHERE is_running = 1)",
            [now],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

#[derive(Debug, Clone, Copy)]
pub struct Delivery {
    pub sent_at: i64,
    pub snooze_until: Option<i64>,
}

#[derive(Debug)]
pub struct Reminder {
    pub date: String,
    pub entry_count: i64,
    pub snooze_minutes: u32,
    pub sound: bool,
}

pub fn reminder_due<Tz: TimeZone>(
    settings: &Settings,
    now: DateTime<Tz>,
    status: ReviewStatus,
    delivery: Option<Delivery>,
) -> bool {
    if !settings.reminders_enabled
        || status == ReviewStatus::Reviewed
        || !settings
            .working_days
            .contains(&now.weekday().number_from_monday())
    {
        return false;
    }
    if let Some(delivery) = delivery {
        let now = now.timestamp_millis();
        return delivery.snooze_until.is_some_and(|until| {
            until >= delivery.sent_at
                && now >= until
                && now.saturating_sub(until) <= TimeDelta::hours(3).num_milliseconds()
        });
    }
    let Ok(time) = NaiveTime::parse_from_str(&settings.reminder_time, "%H:%M") else {
        return false;
    };
    let elapsed = now.time().signed_duration_since(time);
    elapsed >= TimeDelta::zero() && elapsed <= TimeDelta::hours(3)
}

impl Store {
    pub fn claim_task_checks(&mut self, now: i64) -> Result<Vec<TaskCheckReminder>> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let settings = crate::store::read_settings(&tx)?;
        if !settings.task_checks_enabled {
            return Ok(Vec::new());
        }
        let tasks: Vec<_> = crate::tracking::running_entries(&tx)?
            .into_iter()
            .filter(|entry| entry.kind == EntryKind::Activity)
            .collect();
        let mut deliveries: Vec<TaskCheckDelivery> =
            crate::store::read_setting(&tx, "task_check_deliveries")?.unwrap_or_default();
        let original_len = deliveries.len();
        deliveries.retain(|delivery| {
            tasks
                .iter()
                .any(|task| task.id == delivery.task_id && task.activity_at == delivery.started_at)
        });
        let mut reminders = Vec::new();
        for task in tasks {
            let previous = deliveries
                .iter_mut()
                .find(|delivery| delivery.task_id == task.id);
            let checked_at = previous
                .as_ref()
                .map_or(task.activity_at, |delivery| delivery.sent_at);
            if now.saturating_sub(checked_at.max(task.activity_at)) < TASK_CHECK_INTERVAL {
                continue;
            }
            if let Some(delivery) = previous {
                delivery.sent_at = now;
            } else {
                deliveries.push(TaskCheckDelivery {
                    task_id: task.id,
                    started_at: task.activity_at,
                    sent_at: now,
                });
            }
            reminders.push(TaskCheckReminder {
                task,
                sent_at: now,
                sound: settings.notification_sound,
            });
        }
        if !reminders.is_empty() || original_len != deliveries.len() {
            crate::store::write_setting(&tx, "task_check_deliveries", &deliveries)?;
        }
        tx.commit()?;
        Ok(reminders)
    }

    pub fn answer_task_check(
        &mut self,
        reminder: &TaskCheckReminder,
        action: TaskCheckAction,
        now: i64,
    ) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !crate::store::read_settings(&tx)?.task_checks_enabled {
            return Ok(false);
        }
        let current = crate::tracking::running_entries(&tx)?
            .into_iter()
            .find(|task| {
                task.id == reminder.task.id
                    && task.activity_at == reminder.task.activity_at
                    && task.kind == EntryKind::Activity
            });
        let Some(task) = current else {
            return Ok(false);
        };
        let mut deliveries: Vec<TaskCheckDelivery> =
            crate::store::read_setting(&tx, "task_check_deliveries")?.unwrap_or_default();
        let delivery = deliveries.iter_mut().find(|delivery| {
            delivery.task_id == task.id
                && delivery.started_at == task.activity_at
                && delivery.sent_at == reminder.sent_at
        });
        let Some(delivery) = delivery else {
            return Ok(false);
        };
        match action {
            TaskCheckAction::KeepWorking => delivery.sent_at = now,
            TaskCheckAction::SwitchTask => {
                crate::tracking::finish_entry(&tx, &task, now)?;
                deliveries.retain(|delivery| delivery.task_id != task.id);
            }
        }
        crate::store::write_setting(&tx, "task_check_deliveries", &deliveries)?;
        tx.commit()?;
        Ok(true)
    }

    pub fn retry_task_check(&mut self, reminder: &TaskCheckReminder) -> Result<()> {
        let tx = self.conn.transaction()?;
        let mut deliveries: Vec<TaskCheckDelivery> =
            crate::store::read_setting(&tx, "task_check_deliveries")?.unwrap_or_default();
        let original_len = deliveries.len();
        deliveries.retain(|delivery| {
            !(delivery.task_id == reminder.task.id
                && delivery.started_at == reminder.task.activity_at
                && delivery.sent_at == reminder.sent_at)
        });
        if original_len != deliveries.len() {
            crate::store::write_setting(&tx, "task_check_deliveries", &deliveries)?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn claim_task_reminder(&mut self, now: i64) -> Result<Option<TaskReminder>> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let settings = crate::store::read_settings(&tx)?;
        if !settings.task_reminders_enabled {
            return Ok(None);
        }
        let Some((session_id, idle_since)) = idle_workday(&tx, now)? else {
            return Ok(None);
        };
        if now.saturating_sub(idle_since) < TASK_REMINDER_INTERVAL {
            return Ok(None);
        }
        let previous: Option<TaskReminderDelivery> =
            crate::store::read_setting(&tx, "task_reminder_delivery")?;
        if previous.is_some_and(|delivery| {
            delivery.session_id == session_id
                && delivery.idle_since == idle_since
                && now.saturating_sub(delivery.sent_at) < TASK_REMINDER_INTERVAL
        }) {
            return Ok(None);
        }
        crate::store::write_setting(
            &tx,
            "task_reminder_delivery",
            &TaskReminderDelivery {
                session_id,
                idle_since,
                sent_at: now,
            },
        )?;
        tx.commit()?;
        Ok(Some(TaskReminder {
            session_id,
            idle_since,
            sent_at: now,
            sound: settings.notification_sound,
        }))
    }

    pub fn task_reminder_is_current(&self, reminder: &TaskReminder, now: i64) -> Result<bool> {
        Ok(self.settings()?.task_reminders_enabled
            && idle_workday(&self.conn, now)? == Some((reminder.session_id, reminder.idle_since)))
    }

    pub fn retry_task_reminder(&mut self, reminder: &TaskReminder) -> Result<()> {
        let tx = self.conn.transaction()?;
        let previous: Option<TaskReminderDelivery> =
            crate::store::read_setting(&tx, "task_reminder_delivery")?;
        if previous.is_some_and(|delivery| {
            delivery.sent_at == reminder.sent_at && delivery.session_id == reminder.session_id
        }) {
            tx.execute(
                "DELETE FROM settings WHERE key = 'task_reminder_delivery'",
                [],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn claim_reminder(&mut self, now: DateTime<Local>) -> Result<Option<Reminder>> {
        let date = now.format("%Y-%m-%d").to_string();
        let timestamp = now.timestamp_millis();
        let tx = self.conn.transaction()?;
        let settings = crate::store::read_settings(&tx)?;
        if !settings.reminders_enabled {
            return Ok(None);
        }
        let review = read_review(&tx, &date)?;
        let delivery = tx
            .query_row(
                "SELECT sent_at, snooze_until FROM reminder_deliveries WHERE review_date = ?1",
                [&date],
                |row| {
                    Ok(Delivery {
                        sent_at: row.get(0)?,
                        snooze_until: row.get(1)?,
                    })
                },
            )
            .optional()?;
        if !reminder_due(&settings, now, review.status, delivery) {
            return Ok(None);
        }
        let (start, end) = day_bounds(&date)?;
        let entry_count = tx.query_row(
            "SELECT count(*) FROM work_entries WHERE activity_at >= ?1 AND activity_at < ?2",
            [start, end],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO reminder_deliveries (review_date, sent_at) VALUES (?1, ?2)
             ON CONFLICT(review_date) DO UPDATE SET sent_at = excluded.sent_at, snooze_until = NULL",
            params![date, timestamp],
        )?;
        tx.commit()?;
        Ok(Some(Reminder {
            date,
            entry_count,
            snooze_minutes: settings.snooze_minutes,
            sound: settings.notification_sound,
        }))
    }

    pub fn snooze_reminder(&mut self, date: &str, now: DateTime<Local>) -> Result<bool> {
        let settings = self.settings()?;
        let until = now + TimeDelta::minutes(i64::from(settings.snooze_minutes));
        if !settings.reminders_enabled
            || date != now.format("%Y-%m-%d").to_string()
            || now.date_naive() != until.date_naive()
            || self.review(date)?.status == ReviewStatus::Reviewed
        {
            return Ok(false);
        }
        let changed = self.conn.execute(
            "UPDATE reminder_deliveries SET snooze_until = ?1 WHERE review_date = ?2 AND snooze_until IS NULL",
            params![until.timestamp_millis(), date],
        )?;
        Ok(changed == 1)
    }
}
