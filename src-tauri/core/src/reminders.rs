use chrono::{DateTime, Datelike, Local, NaiveTime, TimeDelta, TimeZone};
use rusqlite::{params, OptionalExtension};

use crate::{day_bounds, store::read_review, Result, ReviewStatus, Settings, Store};

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
