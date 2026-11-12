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
        let mut state = TrackingState {
            session: open_session(&self.conn)?,
            tasks: Vec::new(),
            active_break: None,
        };
        for entry in running_entries(&self.conn)? {
            match entry.kind {
                EntryKind::Activity => state.tasks.push(entry),
                EntryKind::Break => state.active_break = Some(entry),
            }
        }
        Ok(state)
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

    pub fn workday_summary(&self, date: &str, now: i64) -> Result<WorkdaySummary> {
        let sessions = self.work_sessions(date)?;
        let work = merge_ranges(
            sessions
                .iter()
                .map(|session| {
                    let end = session
                        .ended_at
                        .unwrap_or_else(|| now.max(session.started_at));
                    (session.started_at, end)
                })
                .collect(),
        );
        let totals = match (work.first(), work.last()) {
            (Some(&(start, _)), Some(&(_, end))) => {
                let mut query = self.conn.prepare(
                    "SELECT activity_at, COALESCE(end_at, ?3), end_at IS NOT NULL FROM work_entries
                     WHERE kind = 'break' AND activity_at < ?2
                       AND (end_at IS NOT NULL OR is_running = 1)
                       AND COALESCE(end_at, ?3) > MAX(?1, activity_at)",
                )?;
                let rows = query.query_map([start, end, now], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, bool>(2)?,
                    ))
                })?;
                let mut ranges = Vec::new();
                let mut completed_breaks = 0;
                for row in rows {
                    let (start, end, finished) = row?;
                    if finished && work.iter().any(|&(from, to)| end.min(to) > start.max(from)) {
                        completed_breaks += 1;
                    }
                    ranges.push((start, end));
                }
                let break_ms = overlap_ms(&work, &merge_ranges(ranges));
                let session_ms: i64 = work.iter().map(|&(start, end)| end - start).sum();
                Some(WorkdayTotals {
                    worked_ms: session_ms - break_ms,
                    break_ms,
                    completed_breaks,
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
                    for entry in running_entries(&tx)? {
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
                let active = running_entries(&tx)?;
                if !active.iter().any(|entry| entry.kind == EntryKind::Break) {
                    ensure_session(&tx, now)?;
                    for entry in active {
                        finish_entry(&tx, &entry, now)?;
                    }
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

pub(crate) fn prepare_task_tracking(tx: &Transaction<'_>, now: i64) -> Result<()> {
    let active = running_entries(tx)?;
    if active.len() >= 2 {
        return Err(invalid(
            "Two tasks are already running. Stop one to start another.",
        ));
    }
    ensure_session(tx, now)?;
    for entry in active.iter().filter(|entry| entry.kind == EntryKind::Break) {
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
    let entry = running_entries(tx)?
        .into_iter()
        .find(|entry| entry.id == id && entry.kind == kind);
    if let Some(entry) = entry {
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

fn running_entries(conn: &Connection) -> Result<Vec<Entry>> {
    let mut query = conn.prepare(&format!(
        "SELECT {ENTRY_COLUMNS} FROM work_entries WHERE is_running = 1 ORDER BY activity_at, id"
    ))?;
    let rows = query.query_map([], map_entry)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

fn map_session(row: &Row<'_>) -> rusqlite::Result<WorkSession> {
    Ok(WorkSession {
        id: row.get(0)?,
        started_at: row.get(1)?,
        ended_at: row.get(2)?,
    })
}
