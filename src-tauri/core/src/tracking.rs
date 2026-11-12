use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction, TransactionBehavior};

use crate::{
    day_bounds, invalid, local_date,
    store::{flag_review, map_entry, ENTRY_COLUMNS},
    Entry, EntryKind, Result, Store, TrackingAction, TrackingState, WorkSession, WorkdaySummary,
    WorkdayTotals,
};

impl Store {
    pub fn tracking_state(&self) -> Result<TrackingState> {
        Ok(TrackingState {
            session: open_session(&self.conn)?,
            entry: running_entry(&self.conn)?,
        })
    }

    pub fn work_sessions(&self, date: &str) -> Result<Vec<WorkSession>> {
        let (start, end) = day_bounds(date)?;
        let mut query = self.conn.prepare(
            "SELECT id, started_at, ended_at FROM work_sessions
             WHERE started_at >= ?1 AND started_at < ?2 ORDER BY started_at, id",
        )?;
        let rows = query.query_map([start, end], map_session)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    pub fn workday_summary(&self, date: &str) -> Result<WorkdaySummary> {
        let sessions = self.work_sessions(date)?;
        let work = merge_ranges(
            sessions
                .iter()
                .filter_map(|session| session.ended_at.map(|end| (session.started_at, end)))
                .collect(),
        );
        let totals = match (work.first(), work.last()) {
            (Some(&(start, _)), Some(&(_, end))) => {
                let mut query = self.conn.prepare(
                    "SELECT activity_at, end_at FROM work_entries
                     WHERE kind = 'break' AND activity_at < ?2 AND end_at > ?1",
                )?;
                let rows = query.query_map([start, end], |row| Ok((row.get(0)?, row.get(1)?)))?;
                let breaks = merge_ranges(rows.collect::<rusqlite::Result<_>>()?);
                let break_ms = overlap_ms(&work, &breaks);
                let session_ms: i64 = work.iter().map(|&(start, end)| end - start).sum();
                Some(WorkdayTotals {
                    worked_ms: session_ms - break_ms,
                    break_ms,
                })
            }
            _ => None,
        };
        Ok(WorkdaySummary { sessions, totals })
    }

    pub fn update_work_session(
        &mut self,
        session: WorkSession,
        started_at: i64,
        ended_at: Option<i64>,
    ) -> Result<()> {
        if ended_at.is_some() != session.ended_at.is_some() {
            return Err(invalid(
                "Use the Workday controls to start or end a work session.",
            ));
        }
        if let Some(end) = ended_at {
            local_date(end)?;
            if end < started_at {
                return Err(invalid("The end time must be at or after the start time."));
            }
        } else if started_at > Utc::now().timestamp_millis() {
            return Err(invalid(
                "A running work session must start by the current time.",
            ));
        }
        let date = local_date(started_at)?;
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        check_session(&tx, &session)?;
        if session.started_at != started_at || session.ended_at != ended_at {
            tx.execute(
                "UPDATE work_sessions SET started_at = ?1, ended_at = ?2 WHERE id = ?3",
                params![started_at, ended_at, session.id],
            )?;
            let previous_date = local_date(session.started_at)?;
            flag_review(&tx, &previous_date)?;
            if previous_date != date {
                flag_review(&tx, &date)?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_work_session(&mut self, session: WorkSession) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        check_session(&tx, &session)?;
        if session.ended_at.is_none() {
            return Err(invalid("End the work session before deleting it."));
        }
        tx.execute("DELETE FROM work_sessions WHERE id = ?1", [session.id])?;
        flag_review(&tx, &local_date(session.started_at)?)?;
        tx.commit()?;
        Ok(())
    }

    pub fn track(&mut self, action: TrackingAction) -> Result<()> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = Utc::now().timestamp_millis();
        match action {
            TrackingAction::StartDay => ensure_session(&tx, now)?,
            TrackingAction::EndDay { id } => {
                if let Some(session) = open_session(&tx)?.filter(|session| session.id == id) {
                    if let Some(entry) = running_entry(&tx)? {
                        finish_entry(&tx, &entry, now)?;
                    }
                    tx.execute(
                        "UPDATE work_sessions SET ended_at = MAX(?1, started_at) WHERE id = ?2",
                        params![now, id],
                    )?;
                    flag_review(&tx, &local_date(session.started_at)?)?;
                }
            }
            TrackingAction::StartBreak => {
                let active = running_entry(&tx)?;
                if !active
                    .as_ref()
                    .is_some_and(|entry| entry.kind == EntryKind::Break)
                {
                    replace_running_entry(&tx, active.as_ref(), now)?;
                    tx.execute(
                        "INSERT INTO work_entries (description, kind, activity_at, created_at, updated_at, is_running)
                         VALUES ('Break', 'break', ?1, ?1, ?1, 1)",
                        [now],
                    )?;
                    flag_review(&tx, &local_date(now)?)?;
                }
            }
            TrackingAction::EndBreak { id } => stop_if_running(&tx, id, EntryKind::Break, now)?,
            TrackingAction::StopTask { id } => stop_if_running(&tx, id, EntryKind::Activity, now)?,
        }
        tx.commit()?;
        Ok(())
    }
}

fn merge_ranges(mut ranges: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    ranges.sort_unstable();
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some((_, previous_end)) if start <= *previous_end => {
                *previous_end = (*previous_end).max(end);
            }
            _ => merged.push((start, end)),
        }
    }
    merged
}

fn overlap_ms(work: &[(i64, i64)], breaks: &[(i64, i64)]) -> i64 {
    let mut break_ranges = breaks.iter().peekable();
    let mut elapsed = 0;
    for &(start, end) in work {
        while let Some(&&(break_start, break_end)) = break_ranges.peek() {
            if break_start >= end {
                break;
            }
            elapsed += (end.min(break_end) - start.max(break_start)).max(0);
            if break_end > end {
                break;
            }
            break_ranges.next();
        }
    }
    elapsed
}

pub(crate) fn prepare_tracking(tx: &Transaction<'_>, now: i64) -> Result<()> {
    replace_running_entry(tx, running_entry(tx)?.as_ref(), now)
}

fn replace_running_entry(tx: &Transaction<'_>, entry: Option<&Entry>, now: i64) -> Result<()> {
    ensure_session(tx, now)?;
    if let Some(entry) = entry {
        finish_entry(tx, entry, now)?;
    }
    Ok(())
}

fn ensure_session(tx: &Transaction<'_>, now: i64) -> Result<()> {
    if open_session(tx)?.is_none() {
        tx.execute("INSERT INTO work_sessions (started_at) VALUES (?1)", [now])?;
        flag_review(tx, &local_date(now)?)?;
    }
    Ok(())
}

fn stop_if_running(tx: &Transaction<'_>, id: i64, kind: EntryKind, now: i64) -> Result<()> {
    if let Some(entry) = running_entry(tx)?.filter(|entry| entry.id == id && entry.kind == kind) {
        finish_entry(tx, &entry, now)?;
    }
    Ok(())
}

fn finish_entry(tx: &Transaction<'_>, entry: &Entry, now: i64) -> Result<()> {
    tx.execute(
        "UPDATE work_entries SET end_at = MAX(?1, activity_at), updated_at = ?1, is_running = 0 WHERE id = ?2",
        params![now, entry.id],
    )?;
    flag_review(tx, &local_date(entry.activity_at)?)
}

fn check_session(conn: &Connection, session: &WorkSession) -> Result<()> {
    let current = conn
        .query_row(
            "SELECT id, started_at, ended_at FROM work_sessions WHERE id = ?1",
            [session.id],
            map_session,
        )
        .optional()?;
    if current.as_ref() != Some(session) {
        return Err(invalid(
            "This work session changed. Close the dialog and try again.",
        ));
    }
    Ok(())
}

fn open_session(conn: &Connection) -> Result<Option<WorkSession>> {
    Ok(conn
        .query_row(
            "SELECT id, started_at, ended_at FROM work_sessions WHERE ended_at IS NULL",
            [],
            map_session,
        )
        .optional()?)
}

fn running_entry(conn: &Connection) -> Result<Option<Entry>> {
    Ok(conn
        .query_row(
            &format!("SELECT {ENTRY_COLUMNS} FROM work_entries WHERE is_running = 1"),
            [],
            map_entry,
        )
        .optional()?)
}

fn map_session(row: &Row<'_>) -> rusqlite::Result<WorkSession> {
    Ok(WorkSession {
        id: row.get(0)?,
        started_at: row.get(1)?,
        ended_at: row.get(2)?,
    })
}
