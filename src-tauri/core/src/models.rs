use chrono::{DateTime, Days, Local, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{invalid, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub minimize_to_tray: bool,
    pub theme: Theme,
    pub time_format: TimeFormat,
    pub hide_after_save: bool,
    pub default_category: Option<i64>,
    pub shortcut: String,
    pub show_confirmation: bool,
    pub remember_category: bool,
    pub reminders_enabled: bool,
    pub reminder_time: String,
    pub working_days: Vec<u32>,
    pub snooze_minutes: u32,
    pub notification_sound: bool,
    pub detect_references: bool,
    pub reference_prefixes: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            minimize_to_tray: true,
            theme: Theme::System,
            time_format: TimeFormat::TwentyFourHour,
            hide_after_save: true,
            default_category: None,
            shortcut: String::new(),
            show_confirmation: true,
            remember_category: false,
            reminders_enabled: false,
            reminder_time: "17:30".into(),
            working_days: vec![1, 2, 3, 4, 5],
            snooze_minutes: 15,
            notification_sound: false,
            detect_references: true,
            reference_prefixes: vec!["NCR".into()],
        }
    }
}

impl Settings {
    pub(crate) fn normalize(&mut self) -> Result<()> {
        if self.reminder_time.len() != 5
            || NaiveTime::parse_from_str(&self.reminder_time, "%H:%M").is_err()
        {
            return Err(invalid("Enter a valid reminder time."));
        }
        if self.working_days.iter().any(|day| !(1..=7).contains(day))
            || (self.reminders_enabled && self.working_days.is_empty())
        {
            return Err(invalid("Select at least one working day for reminders."));
        }
        if !(1..=180).contains(&self.snooze_minutes) {
            return Err(invalid(
                "Snooze duration must be between 1 and 180 minutes.",
            ));
        }
        self.working_days.sort_unstable();
        self.working_days.dedup();
        self.shortcut = self.shortcut.trim().to_owned();
        if self.shortcut.len() > 80 || self.shortcut.chars().any(char::is_control) {
            return Err(invalid("Enter a valid keyboard shortcut."));
        }
        if self.reference_prefixes.len() > 32 {
            return Err(invalid("Use up to 32 reference prefixes."));
        }
        for prefix in &mut self.reference_prefixes {
            *prefix = prefix.trim().to_ascii_uppercase();
            if !(1..=16).contains(&prefix.len())
                || !prefix.as_bytes()[0].is_ascii_alphabetic()
                || !prefix.bytes().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(invalid("Reference prefixes must start with a letter and contain up to 16 letters or digits."));
            }
        }
        self.reference_prefixes.sort_unstable();
        self.reference_prefixes.dedup();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TimeFormat {
    #[serde(rename = "12h")]
    TwelveHour,
    #[serde(rename = "24h")]
    TwentyFourHour,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub sort_order: i64,
    pub is_active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    pub id: Option<i64>,
    pub name: String,
    pub color: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub prefix: String,
    pub number: String,
}

pub fn detect_reference(description: &str, prefixes: &[String]) -> Option<Reference> {
    if prefixes.is_empty() {
        return None;
    }
    let prefixes = prefixes
        .iter()
        .map(|prefix| regex::escape(prefix))
        .collect::<Vec<_>>();
    let pattern = Regex::new(&format!(
        r"(?i)\b({})[ \t]*(?:-[ \t]*)?([0-9]{{1,24}})\b",
        prefixes.join("|")
    ))
    .ok()?;
    let found = pattern.captures(description)?;
    Some(Reference {
        prefix: found[1].to_ascii_uppercase(),
        number: found[2].to_owned(),
    })
}

impl Reference {
    pub(crate) fn normalize(&mut self) -> Result<()> {
        self.prefix = self.prefix.trim().to_ascii_uppercase();
        self.number = self.number.trim().to_owned();
        if !(1..=16).contains(&self.prefix.len())
            || !self.prefix.as_bytes()[0].is_ascii_alphabetic()
            || !self.prefix.bytes().all(|c| c.is_ascii_alphanumeric())
            || !(1..=24).contains(&self.number.len())
            || !self.number.bytes().all(|c| c.is_ascii_digit())
        {
            return Err(invalid(
                "Enter a reference prefix and a number, or leave both blank.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EntryKind {
    #[default]
    Activity,
    Break,
}

impl EntryKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Activity => "activity",
            Self::Break => "break",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub description: String,
    pub reference: Option<Reference>,
    pub category_id: Option<i64>,
    pub activity_at: i64,
    pub end_at: Option<i64>,
    pub kind: EntryKind,
    pub is_running: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub description: String,
    pub category_id: Option<i64>,
    pub activity_at: Option<i64>,
    #[serde(default)]
    pub end_at: Option<i64>,
    #[serde(default)]
    pub update_end: bool,
    #[serde(default)]
    pub kind: EntryKind,
    pub reference: Option<Reference>,
    #[serde(default)]
    pub auto_reference: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkSession {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkdaySummary {
    pub sessions: Vec<WorkSession>,
    pub totals: Option<WorkdayTotals>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkdayTotals {
    pub worked_ms: i64,
    pub break_ms: i64,
    pub completed_breaks: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackingState {
    pub session: Option<WorkSession>,
    pub tasks: Vec<Entry>,
    pub active_break: Option<Entry>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TrackingAction {
    StartDay,
    EndDay { id: i64 },
    StartBreak,
    EndBreak { id: i64 },
    StopTask { id: i64 },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryQuery {
    pub date: Option<String>,
    #[serde(default)]
    pub search: String,
    pub category_id: Option<i64>,
    #[serde(default)]
    pub uncategorized: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ReviewStatus {
    Open,
    Reviewed,
    NeedsReview,
}

impl ReviewStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Reviewed => "reviewed",
            Self::NeedsReview => "needs_review",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub date: String,
    pub notes: String,
    pub status: ReviewStatus,
    pub reviewed_at: Option<i64>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum ReviewAction {
    SaveNotes,
    MarkReviewed,
    Reopen,
}

pub(crate) fn parse_date(date: &str) -> Result<NaiveDate> {
    let parsed =
        NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| invalid("Enter a valid date."))?;
    if date.len() != 10 || parsed.format("%Y-%m-%d").to_string() != date {
        return Err(invalid("Enter a valid date."));
    }
    Ok(parsed)
}

pub(crate) fn utc_timestamp(value: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp_millis(value).ok_or_else(|| invalid("Enter a valid timestamp."))
}

pub(crate) fn local_date(value: i64) -> Result<String> {
    let date = utc_timestamp(value)?
        .with_timezone(&Local)
        .format("%Y-%m-%d")
        .to_string();
    parse_date(&date)?;
    Ok(date)
}

fn local_day_start(date: NaiveDate) -> Result<i64> {
    let midnight = date.and_time(NaiveTime::MIN);
    // A zone can advance at midnight, or even skip an entire civil date.
    for offset in 0..=2 * 24 * 60 {
        let Some(time) = midnight.checked_add_signed(TimeDelta::minutes(offset)) else {
            break;
        };
        if let Some(instant) = Local.from_local_datetime(&time).earliest() {
            return Ok(instant.timestamp_millis());
        }
    }
    Err(invalid("This date is outside the supported time range."))
}

pub(crate) fn day_bounds(date: &str) -> Result<(i64, i64)> {
    let day = parse_date(date)?;
    let next = day
        .checked_add_days(Days::new(1))
        .ok_or_else(|| invalid("Enter a valid date."))?;
    Ok((local_day_start(day)?, local_day_start(next)?))
}
