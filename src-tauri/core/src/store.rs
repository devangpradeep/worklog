use std::{path::Path, time::Duration};

use chrono::Utc;
use rusqlite::{
    functions::FunctionFlags, params, Connection, OptionalExtension, Row, Transaction,
    TransactionBehavior,
};

use crate::{
    day_bounds, detect_reference, invalid, local_date, parse_date, Category, CategoryInput, Entry,
    EntryInput, EntryKind, EntryQuery, Reference, Result, Review, ReviewAction, ReviewStatus,
    Settings,
};

pub(crate) const APPLICATION_ID: i64 = 0x574C4F47;
pub(crate) const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/001_initial.sql"),
    include_str!("../migrations/002_capture_and_reminders.sql"),
    include_str!("../migrations/003_workday_tracking.sql"),
    include_str!("../migrations/004_parallel_tasks.sql"),
];
pub(crate) const ENTRY_COLUMNS: &str = "id, description, reference_type, reference_number,
    category_id, activity_at, created_at, updated_at, end_at, kind, is_running";

pub struct Store {
    pub(crate) conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        Self::initialize(Connection::open(path)?)
    }

    pub(crate) fn initialize(mut conn: Connection) -> Result<Self> {
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA trusted_schema = OFF;")?;
        migrate(&mut conn)?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = FULL;")?;
        conn.create_scalar_function(
            "fold_case",
            1,
            FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
            |context| Ok(context.get::<String>(0)?.to_lowercase()),
        )?;
        Ok(Self { conn })
    }

    pub fn settings(&self) -> Result<Settings> {
        read_settings(&self.conn)
    }

    pub fn save_settings(&mut self, mut settings: Settings) -> Result<Settings> {
        settings.normalize()?;
        let tx = self.conn.transaction()?;
        check_category(&tx, settings.default_category, None)?;
        write_setting(&tx, "preferences", &settings)?;
        tx.commit()?;
        Ok(settings)
    }

    pub fn capture_category(&self, settings: &Settings) -> Result<Option<i64>> {
        if settings.remember_category {
            if let Some(category) =
                read_setting::<Option<i64>>(&self.conn, "last_capture_category")?
            {
                match category {
                    None => return Ok(None),
                    Some(id) if category_active(&self.conn, id)? => return Ok(Some(id)),
                    _ => {}
                }
            }
        }
        match settings.default_category {
            Some(id) if category_active(&self.conn, id)? => Ok(Some(id)),
            _ => Ok(None),
        }
    }

    pub fn categories(&self) -> Result<Vec<Category>> {
        let mut query = self.conn.prepare(
            "SELECT id, name, color, sort_order, is_active FROM categories ORDER BY sort_order, id",
        )?;
        let rows = query.query_map([], |row| {
            Ok(Category {
                id: row.get(0)?,
                name: row.get(1)?,
                color: row.get(2)?,
                sort_order: row.get(3)?,
                is_active: row.get(4)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_category(&mut self, input: CategoryInput) -> Result<()> {
        let name = input.name.trim();
        let color = input.color.as_bytes();
        if name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
            return Err(invalid("Enter a category name of up to 60 characters."));
        }
        if color.len() != 7 || color[0] != b'#' || !color[1..].iter().all(u8::is_ascii_hexdigit) {
            return Err(invalid("Choose a valid category color."));
        }
        let tx = self.conn.transaction()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE fold_case(name) = ?1 AND id IS NOT ?2)",
            params![name.to_lowercase(), input.id],
            |row| row.get(0),
        )?;
        if exists {
            return Err(invalid("A category with that name already exists."));
        }
        if let Some(id) = input.id {
            let count = tx.execute(
                "UPDATE categories SET name = ?1, color = ?2, is_active = ?3 WHERE id = ?4",
                params![name, input.color, input.is_active, id],
            )?;
            if count == 0 {
                return Err(invalid("This category no longer exists."));
            }
            if !input.is_active {
                let mut settings = read_settings(&tx)?;
                if settings.default_category == Some(id) {
                    settings.default_category = None;
                    write_setting(&tx, "preferences", &settings)?;
                }
            }
        } else {
            tx.execute(
                "INSERT INTO categories (name, color, is_active, sort_order)
                 SELECT ?1, ?2, ?3, COALESCE(MAX(sort_order) + 1, 0) FROM categories",
                params![name, input.color, input.is_active],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn reorder_categories(&mut self, ids: Vec<i64>) -> Result<()> {
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        let mut current: Vec<i64> = self
            .categories()?
            .into_iter()
            .map(|category| category.id)
            .collect();
        current.sort_unstable();
        if sorted != current {
            return Err(invalid(
                "The categories changed. Reload their order and try again.",
            ));
        }
        let tx = self.conn.transaction()?;
        for (order, id) in ids.into_iter().enumerate() {
            tx.execute(
                "UPDATE categories SET sort_order = ?1 WHERE id = ?2",
                params![order as i64, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn entry(&self, id: i64) -> Result<Entry> {
        read_entry(&self.conn, id)
    }

    pub fn entries(&self, query: EntryQuery) -> Result<Vec<Entry>> {
        let bounds = query.date.as_deref().map(day_bounds).transpose()?;
        let search = query.search.trim().to_lowercase();
        let compact = search.replace([' ', '-'], "");
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENTRY_COLUMNS} FROM work_entries
             WHERE activity_at >= ?1 AND activity_at < ?2
               AND (?3 IS NULL OR category_id = ?3)
               AND (NOT ?4 OR category_id IS NULL)
               AND (?5 = '' OR instr(fold_case(description), ?5) > 0
                 OR (?6 <> '' AND instr(lower(reference_type || reference_number), ?6) > 0))
             ORDER BY activity_at, id",
        ))?;
        let (start, end) = bounds.unwrap_or((i64::MIN, i64::MAX));
        let rows = stmt.query_map(
            params![
                start,
                end,
                query.category_id,
                query.uncategorized,
                search,
                compact
            ],
            map_entry,
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_entry(
        &mut self,
        id: Option<i64>,
        input: EntryInput,
        capture_key: Option<&str>,
    ) -> Result<Entry> {
        self.write_entry(id, input, capture_key, false)
    }

    pub fn start_task(&mut self, input: EntryInput, capture_key: &str) -> Result<Entry> {
        self.write_entry(None, input, Some(capture_key), true)
    }

    fn write_entry(
        &mut self,
        id: Option<i64>,
        mut input: EntryInput,
        capture_key: Option<&str>,
        start_task: bool,
    ) -> Result<Entry> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = Utc::now().timestamp_millis();
        if let Some(key) = capture_key {
            if id.is_some() || key.len() != 32 || !key.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(invalid(
                    "Quick Capture sent an invalid request. Reopen WorkLog and try again.",
                ));
            }
            if let Some(existing) = tx
                .query_row(
                    "SELECT id FROM work_entries WHERE capture_key = ?1",
                    [key],
                    |row| row.get(0),
                )
                .optional()?
            {
                return read_entry(&tx, existing);
            }
        }
        let description = input.description.trim();
        let description = if input.kind == EntryKind::Break && description.is_empty() {
            "Break"
        } else {
            description
        };
        if input.kind == EntryKind::Break {
            input.category_id = None;
            input.reference = None;
            input.auto_reference = false;
        }
        if description.is_empty()
            || description.chars().count() > 4000
            || description.contains('\0')
        {
            return Err(invalid("Enter an activity between 1 and 4,000 characters."));
        }

        let previous = id.map(|id| read_entry(&tx, id)).transpose()?;
        let retained_category = previous.as_ref().and_then(|entry| entry.category_id);
        check_category(&tx, input.category_id, retained_category)?;

        let settings = read_settings(&tx)?;
        if input.auto_reference && settings.detect_references {
            input.reference = detect_reference(description, &settings.reference_prefixes);
        }
        if let Some(reference) = &mut input.reference {
            reference.normalize()?;
        }
        let prefix = input.reference.as_ref().map(|value| &value.prefix);
        let number = input.reference.as_ref().map(|value| &value.number);
        let activity_at = if start_task {
            now
        } else {
            input
                .activity_at
                .or_else(|| previous.as_ref().map(|entry| entry.activity_at))
                .unwrap_or(now)
        };
        let date = local_date(activity_at)?;
        let end_at = if previous.is_none() || input.update_end {
            input.end_at
        } else {
            previous.as_ref().and_then(|entry| entry.end_at)
        };
        if let Some(end_at) = end_at {
            local_date(end_at)?;
            if end_at < activity_at {
                return Err(invalid("The end time must be at or after the start time."));
            }
        }
        let is_running = start_task
            || (end_at.is_none() && previous.as_ref().is_some_and(|entry| entry.is_running));
        if is_running && (activity_at > now || end_at.is_some()) {
            return Err(invalid(
                "A running activity must start by the current time and have no end time.",
            ));
        }

        let id = if let Some(previous) = previous {
            if previous.description == description
                && previous.category_id == input.category_id
                && previous.reference == input.reference
                && previous.activity_at == activity_at
                && previous.end_at == end_at
                && previous.kind == input.kind
            {
                return Ok(previous);
            }
            tx.execute(
                "UPDATE work_entries SET description = ?1, reference_type = ?2, reference_number = ?3,
                   category_id = ?4, activity_at = ?5, updated_at = ?6,
                   end_at = ?7, kind = ?8, is_running = ?9 WHERE id = ?10",
                params![description, prefix, number, input.category_id, activity_at, now,
                    end_at, input.kind.as_str(), is_running, previous.id],
            )?;
            let old_date = local_date(previous.activity_at)?;
            if old_date != date {
                flag_review(&tx, &old_date)?;
            }
            previous.id
        } else {
            if start_task {
                crate::tracking::prepare_task_tracking(&tx, now)?;
            }
            tx.execute(
                "INSERT INTO work_entries (description, reference_type, reference_number, category_id,
                   activity_at, created_at, updated_at, capture_key, end_at, kind, is_running)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, ?8, ?9, ?10)",
                params![description, prefix, number, input.category_id, activity_at, now,
                    capture_key, end_at, input.kind.as_str(), is_running],
            )?;
            let id = tx.last_insert_rowid();
            if capture_key.is_some() {
                write_setting(&tx, "last_capture_category", &input.category_id)?;
            }
            id
        };
        flag_review(&tx, &date)?;
        let entry = read_entry(&tx, id)?;
        tx.commit()?;
        Ok(entry)
    }

    pub fn delete_entry(&mut self, id: i64) -> Result<()> {
        let tx = self.conn.transaction()?;
        let entry = read_entry(&tx, id)?;
        tx.execute("DELETE FROM work_entries WHERE id = ?1", [id])?;
        flag_review(&tx, &local_date(entry.activity_at)?)?;
        tx.commit()?;
        Ok(())
    }

    pub fn review(&self, date: &str) -> Result<Review> {
        parse_date(date)?;
        read_review(&self.conn, date)
    }

    pub fn save_review(&mut self, date: &str, notes: &str, action: ReviewAction) -> Result<Review> {
        parse_date(date)?;
        if notes.chars().count() > 10000 || notes.contains('\0') {
            return Err(invalid("Review notes can contain up to 10,000 characters."));
        }
        let tx = self.conn.transaction()?;
        let previous = read_review(&tx, date)?;
        let (status, reviewed_at) = match action {
            ReviewAction::SaveNotes => (previous.status, previous.reviewed_at),
            ReviewAction::MarkReviewed => {
                (ReviewStatus::Reviewed, Some(Utc::now().timestamp_millis()))
            }
            ReviewAction::Reopen => (ReviewStatus::Open, None),
        };
        tx.execute(
            "INSERT INTO daily_reviews (review_date, notes, status, reviewed_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(review_date) DO UPDATE SET notes = excluded.notes,
                 status = excluded.status, reviewed_at = excluded.reviewed_at",
            params![date, notes, status.as_str(), reviewed_at],
        )?;
        if status == ReviewStatus::Reviewed {
            tx.execute(
                "UPDATE reminder_deliveries SET snooze_until = NULL WHERE review_date = ?1",
                [date],
            )?;
        }
        tx.commit()?;
        Ok(Review {
            date: date.into(),
            notes: notes.into(),
            status,
            reviewed_at,
        })
    }

    pub fn reviews(&self, month: &str) -> Result<Vec<Review>> {
        parse_date(&format!("{month}-01"))?;
        let mut stmt = self.conn.prepare(
            "SELECT review_date, notes, status, reviewed_at FROM daily_reviews WHERE review_date LIKE ?1 ORDER BY review_date",
        )?;
        let rows = stmt.query_map([format!("{month}-%")], map_review)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

pub(crate) fn migrate(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let id: i64 = tx.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: usize = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if id != APPLICATION_ID && (id != 0 || version != 0) {
        return Err(invalid("This file is not a WorkLog database."));
    }
    if version > MIGRATIONS.len() {
        return Err(invalid(
            "This database requires a newer version of WorkLog.",
        ));
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(version) {
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index + 1)?;
    }
    tx.pragma_update(None, "application_id", APPLICATION_ID)?;
    tx.commit()?;
    Ok(())
}

fn read_setting<T: serde::de::DeserializeOwned>(conn: &Connection, key: &str) -> Result<Option<T>> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()?;
    Ok(value.as_deref().map(serde_json::from_str).transpose()?)
}

fn write_setting(conn: &Connection, key: &str, value: &impl serde::Serialize) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, serde_json::to_string(value)?],
    )?;
    Ok(())
}

pub(crate) fn read_settings(conn: &Connection) -> Result<Settings> {
    Ok(read_setting(conn, "preferences")?.unwrap_or_default())
}

fn category_active(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn
        .query_row(
            "SELECT is_active FROM categories WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(false))
}

fn check_category(conn: &Connection, category: Option<i64>, retained: Option<i64>) -> Result<()> {
    if let Some(id) = category {
        if category != retained && !category_active(conn, id)? {
            return Err(invalid(
                "Choose an active category or leave the activity uncategorized.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn map_entry(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let prefix: Option<String> = row.get(2)?;
    let number: Option<String> = row.get(3)?;
    Ok(Entry {
        id: row.get(0)?,
        description: row.get(1)?,
        reference: prefix
            .zip(number)
            .map(|(prefix, number)| Reference { prefix, number }),
        category_id: row.get(4)?,
        activity_at: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        end_at: row.get(8)?,
        kind: if row.get::<_, String>(9)? == "break" {
            EntryKind::Break
        } else {
            EntryKind::Activity
        },
        is_running: row.get(10)?,
    })
}

fn read_entry(conn: &Connection, id: i64) -> Result<Entry> {
    conn.query_row(
        &format!("SELECT {ENTRY_COLUMNS} FROM work_entries WHERE id = ?1"),
        [id],
        map_entry,
    )
    .optional()?
    .ok_or_else(|| invalid("This activity no longer exists."))
}

fn map_review(row: &Row<'_>) -> rusqlite::Result<Review> {
    let status: String = row.get(2)?;
    Ok(Review {
        date: row.get(0)?,
        notes: row.get(1)?,
        status: match status.as_str() {
            "reviewed" => ReviewStatus::Reviewed,
            "needs_review" => ReviewStatus::NeedsReview,
            _ => ReviewStatus::Open,
        },
        reviewed_at: row.get(3)?,
    })
}

pub(crate) fn read_review(conn: &Connection, date: &str) -> Result<Review> {
    Ok(conn.query_row(
        "SELECT review_date, notes, status, reviewed_at FROM daily_reviews WHERE review_date = ?1",
        [date], map_review,
    ).optional()?.unwrap_or_else(|| Review {
        date: date.into(), notes: String::new(), status: ReviewStatus::Open, reviewed_at: None,
    }))
}

pub(crate) fn flag_review(tx: &Transaction<'_>, date: &str) -> Result<()> {
    tx.execute(
        "UPDATE daily_reviews SET status = 'needs_review' WHERE review_date = ?1 AND status = 'reviewed'",
        [date],
    )?;
    Ok(())
}
