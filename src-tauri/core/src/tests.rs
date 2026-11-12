use chrono::{Local, TimeDelta, TimeZone, Utc};
use rusqlite::{params, Connection};

use crate::{
    day_bounds, detect_reference, local_date, reminder_due, store::APPLICATION_ID,
    store::MIGRATIONS, CategoryInput, Delivery, EntryInput, EntryQuery, Reference, ReviewAction,
    ReviewStatus, Settings, Store, Theme,
};

fn database() -> Store {
    Store::initialize(Connection::open_in_memory().unwrap()).unwrap()
}

fn input(description: &str, activity_at: Option<i64>) -> EntryInput {
    EntryInput {
        description: description.into(),
        category_id: None,
        activity_at,
        end_at: None,
        update_end: false,
        kind: crate::EntryKind::Activity,
        reference: None,
        auto_reference: true,
    }
}

fn all(search: &str) -> EntryQuery {
    EntryQuery {
        date: None,
        search: search.into(),
        category_id: None,
        uncategorized: false,
    }
}

#[test]
fn creates_and_deduplicates_quick_capture_with_utc_timestamps() {
    let mut db = database();
    let before = Utc::now().timestamp_millis();
    let key = "99cc122cee1144dfa13b550bc382003f";
    let text = "NCR 2456 - Tested and verified";
    let entry = db.save_entry(None, input(text, None), Some(key)).unwrap();
    let retry = db.save_entry(None, input(text, None), Some(key)).unwrap();
    assert_eq!(entry.id, retry.id);
    assert_eq!(entry.description, text);
    assert_eq!(entry.activity_at, entry.created_at);
    assert_eq!(entry.created_at, entry.updated_at);
    assert!((before..=Utc::now().timestamp_millis()).contains(&entry.activity_at));
    assert_eq!(db.entries(all("ncr-2456")).unwrap().len(), 1);
    assert_eq!(db.entries(all("2456")).unwrap().len(), 1);
    assert!(db.save_entry(None, input("   ", None), None).is_err());
}

#[test]
fn parses_configurable_references() {
    let prefixes = vec!["NCR".into(), "ISSUE".into()];
    for description in ["NCR 2456", "ncr-2456 - checked", "NCR2456"] {
        let found = detect_reference(description, &prefixes).unwrap();
        assert_eq!(
            found,
            Reference {
                prefix: "NCR".into(),
                number: "2456".into()
            }
        );
    }
    assert_eq!(
        detect_reference("Investigated ISSUE-0007", &prefixes)
            .unwrap()
            .number,
        "0007"
    );
    for description in ["ANCR 2456", "NCR 12abc", "No reference here", "BUG-12"] {
        assert!(detect_reference(description, &prefixes).is_none());
    }
    let mut db = database();
    db.save_settings(Settings {
        detect_references: false,
        ..Settings::default()
    })
    .unwrap();
    assert!(db
        .save_entry(None, input("NCR-12", None), None)
        .unwrap()
        .reference
        .is_none());
}

#[test]
fn edits_deletes_and_queries_entries_by_local_date_and_category() {
    let mut db = database();
    let (start, end) = day_bounds("2026-06-15").unwrap();
    let entry = db
        .save_entry(None, input("Release", Some(end - 1)), None)
        .unwrap();
    db.save_entry(None, input("Different day", Some(end)), None)
        .unwrap();
    let mut edit = input("Déploiement", Some(start));
    edit.category_id = Some(6);
    edit.reference = Some(Reference {
        prefix: "ops".into(),
        number: "012".into(),
    });
    edit.auto_reference = false;
    let edited = db.save_entry(Some(entry.id), edit, None).unwrap();
    assert_eq!(edited.reference.unwrap().prefix, "OPS");
    assert_eq!(edited.created_at, entry.created_at);
    assert_eq!(edited.activity_at, start);
    let matching = db
        .entries(EntryQuery {
            date: Some("2026-06-15".into()),
            category_id: Some(6),
            ..all("DÉPLOIEMENT")
        })
        .unwrap();
    assert_eq!(matching.len(), 1);
    db.delete_entry(entry.id).unwrap();
    assert!(db.entry(entry.id).is_err());
    assert_eq!(db.entries(all("")).unwrap().len(), 1);
}

#[test]
fn reviews_are_unique_and_edits_flag_both_dates() {
    let mut db = database();
    let (start, end) = day_bounds("2026-06-15").unwrap();
    let first = local_date(start).unwrap();
    let second = local_date(end).unwrap();
    let entry = db
        .save_entry(None, input("Investigated", Some(start)), None)
        .unwrap();
    for date in [&first, &second] {
        assert_eq!(db.review(date).unwrap().status, ReviewStatus::Open);
        let reviewed = db
            .save_review(date, "Some notes", ReviewAction::MarkReviewed)
            .unwrap();
        assert_eq!(reviewed.status, ReviewStatus::Reviewed);
        assert!(reviewed.reviewed_at.is_some());
    }
    db.save_entry(Some(entry.id), input("Investigated", Some(start)), None)
        .unwrap();
    assert_eq!(db.review(&first).unwrap().status, ReviewStatus::Reviewed);
    db.save_entry(Some(entry.id), input("Verified", Some(end)), None)
        .unwrap();
    for date in [&first, &second] {
        let review = db.review(date).unwrap();
        assert_eq!(review.status, ReviewStatus::NeedsReview);
        assert_eq!(review.notes, "Some notes");
    }
    let reopened = db
        .save_review(&first, "New notes", ReviewAction::Reopen)
        .unwrap();
    assert_eq!(reopened.status, ReviewStatus::Open);
    assert_eq!(reopened.reviewed_at, None);
    assert_eq!(db.reviews("2026-06").unwrap().len(), 2);
}

#[test]
fn categories_and_preferences_persist_without_losing_existing_entries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("worklog.sqlite3");
    let mut db = Store::open(&path).unwrap();
    let saved = db
        .save_settings(Settings {
            theme: Theme::Dark,
            default_category: Some(2),
            remember_category: true,
            working_days: vec![3, 1, 1],
            reference_prefixes: vec!["ops".into(), "Ncr".into()],
            ..Settings::default()
        })
        .unwrap();
    assert_eq!(saved.working_days, vec![1, 3]);
    assert_eq!(saved.reference_prefixes, vec!["NCR", "OPS"]);
    let mut entry = input("Existing entry", None);
    entry.category_id = Some(2);
    let entry = db
        .save_entry(None, entry, Some("aabb122cee1144dfa13b550bc382003f"))
        .unwrap();
    drop(db);

    let mut db = Store::open(&path).unwrap();
    assert_eq!(db.settings().unwrap(), saved);
    assert_eq!(db.capture_category(&saved).unwrap(), Some(2));
    db.save_category(CategoryInput {
        id: Some(2),
        name: "QA".into(),
        color: "#123456".into(),
        is_active: false,
    })
    .unwrap();
    assert_eq!(db.capture_category(&db.settings().unwrap()).unwrap(), None);
    assert_eq!(db.entry(entry.id).unwrap().category_id, Some(2));
    let mut ids: Vec<i64> = db.categories().unwrap().into_iter().map(|c| c.id).collect();
    ids.reverse();
    db.reorder_categories(ids.clone()).unwrap();
    assert_eq!(db.categories().unwrap()[0].id, ids[0]);
}

#[test]
fn migrates_v1_database_and_rejects_future_or_failed_migrations() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1.sqlite3");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(MIGRATIONS[0]).unwrap();
    conn.pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    conn.execute(
        "INSERT INTO work_entries(description, activity_at, created_at, updated_at) VALUES ('Old entry', ?1, ?1, ?1)",
        [Utc::now().timestamp_millis()],
    ).unwrap();
    drop(conn);
    let db = Store::open(&path).unwrap();
    assert_eq!(db.entries(all("")).unwrap()[0].description, "Old entry");
    assert_eq!(db.categories().unwrap().len(), 8);
    let version = |c: &Connection| {
        c.pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
            .unwrap()
    };
    assert_eq!(version(&db.conn), MIGRATIONS.len() as i64);
    db.conn.pragma_update(None, "user_version", 99).unwrap();
    drop(db);
    assert!(Store::open(&path).is_err());
    assert_eq!(version(&Connection::open(path).unwrap()), 99);

    let mut damaged = Connection::open_in_memory().unwrap();
    damaged
        .execute_batch("CREATE TABLE settings (key TEXT);")
        .unwrap();
    assert!(crate::store::migrate(&mut damaged).is_err());
    assert_eq!(version(&damaged), 0);
    assert!(!damaged
        .prepare("SELECT name FROM sqlite_schema WHERE name = 'categories'")
        .unwrap()
        .exists([])
        .unwrap());
}

#[test]
fn schedules_reminders_with_a_bounded_same_day_catch_up() {
    let settings = Settings {
        reminders_enabled: true,
        ..Settings::default()
    };
    let date = Utc.with_ymd_and_hms(2026, 6, 15, 17, 30, 0).unwrap();
    let due = |now, status, delivery| reminder_due(&settings, now, status, delivery);
    assert!(!due(date - TimeDelta::minutes(1), ReviewStatus::Open, None));
    assert!(due(date + TimeDelta::minutes(90), ReviewStatus::Open, None));
    assert!(!due(date + TimeDelta::hours(4), ReviewStatus::Open, None));
    assert!(!due(date + TimeDelta::days(5), ReviewStatus::Open, None));
    assert!(!due(date, ReviewStatus::Reviewed, None));
    let mut delivery = Delivery {
        sent_at: date.timestamp_millis(),
        snooze_until: None,
    };
    assert!(!due(
        date + TimeDelta::minutes(1),
        ReviewStatus::Open,
        Some(delivery)
    ));
    delivery.snooze_until = Some((date + TimeDelta::minutes(15)).timestamp_millis());
    assert!(!due(
        date + TimeDelta::minutes(14),
        ReviewStatus::Open,
        Some(delivery)
    ));
    assert!(due(
        date + TimeDelta::minutes(15),
        ReviewStatus::Open,
        Some(delivery)
    ));
}

#[test]
fn persists_reminder_delivery_and_cancels_snooze_when_reviewed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("worklog.sqlite3");
    let mut db = Store::open(&path).unwrap();
    db.save_settings(Settings {
        reminders_enabled: true,
        ..Settings::default()
    })
    .unwrap();
    let now = Local.with_ymd_and_hms(2026, 6, 15, 17, 31, 0).unwrap();
    let reminder = db.claim_reminder(now).unwrap().unwrap();
    assert_eq!(reminder.entry_count, 0);
    drop(db);
    let mut db = Store::open(&path).unwrap();
    assert!(db.claim_reminder(now).unwrap().is_none());
    assert!(db.snooze_reminder(&reminder.date, now).unwrap());
    assert!(!db.snooze_reminder(&reminder.date, now).unwrap());
    db.save_review(&reminder.date, "", ReviewAction::MarkReviewed)
        .unwrap();
    db.save_review(&reminder.date, "", ReviewAction::Reopen)
        .unwrap();
    assert!(db
        .claim_reminder(now + TimeDelta::minutes(16))
        .unwrap()
        .is_none());
}

#[test]
fn snapshots_wal_restores_safely_and_exports_csv() {
    let temp = tempfile::tempdir().unwrap();
    let live = temp.path().join("worklog.sqlite3");
    let backup = temp.path().join("backup.sqlite3");
    let csv = temp.path().join("entries.csv");
    let mut db = Store::open(&live).unwrap();
    db.save_entry(None, input("Checked \"upgrade\", on staging", None), None)
        .unwrap();
    db.backup(&backup).unwrap();
    assert!(db.backup(&live).is_err());
    db.save_entry(None, input("Later", None), None).unwrap();
    db.restore(&backup).unwrap();
    assert_eq!(db.entries(all("")).unwrap().len(), 1);
    db.export_csv(&csv).unwrap();
    let record = csv::Reader::from_path(csv)
        .unwrap()
        .records()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(&record[1], "Checked \"upgrade\", on staging");
    assert!(record[5].ends_with("+00:00"));

    let invalid = temp.path().join("invalid.sqlite3");
    let foreign = Connection::open(&invalid).unwrap();
    foreign
        .execute("CREATE TABLE unrelated (id INTEGER)", params![])
        .unwrap();
    assert!(db.restore(&invalid).is_err());
    assert_eq!(db.entries(all("")).unwrap().len(), 1);
}

#[test]
fn tracks_workday_task_and_break_across_restart() {
    use crate::{EntryKind, TrackingAction};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("worklog.sqlite3");
    let mut db = Store::open(&path).unwrap();
    assert!(db.tracking_state().unwrap().session.is_none());

    db.track(TrackingAction::StartDay).unwrap();
    let session = db.tracking_state().unwrap().session.unwrap();
    db.track(TrackingAction::StartDay).unwrap();
    assert_eq!(db.tracking_state().unwrap().session.unwrap().id, session.id);

    let key = "7c03125bdecd4bcbb158922fc6dc927e";
    let task = db
        .start_task(input("NCR 876 - Investigate", None), key)
        .unwrap();
    assert!(task.is_running);
    assert_eq!(task.reference.unwrap().number, "876");
    let point = db
        .save_entry(None, input("Noted a detail", None), None)
        .unwrap();
    assert!(!point.is_running && point.end_at.is_none());
    assert_eq!(db.tracking_state().unwrap().tasks[0].id, task.id);

    db.track(TrackingAction::StartBreak).unwrap();
    let pause = db.tracking_state().unwrap().active_break.unwrap();
    assert_eq!(pause.kind, EntryKind::Break);
    assert_eq!(db.entry(task.id).unwrap().end_at, Some(pause.activity_at));
    db.track(TrackingAction::StartBreak).unwrap();
    assert_eq!(
        db.tracking_state().unwrap().active_break.unwrap().id,
        pause.id
    );
    db.start_task(input("Retry same capture", None), key)
        .unwrap();
    assert_eq!(
        db.tracking_state().unwrap().active_break.unwrap().id,
        pause.id
    );
    drop(db);

    let mut db = Store::open(&path).unwrap();
    assert_eq!(
        db.tracking_state().unwrap().active_break.unwrap().id,
        pause.id
    );
    db.track(TrackingAction::EndBreak { id: pause.id }).unwrap();
    assert!(db.entry(pause.id).unwrap().end_at.is_some());
    let task = db
        .start_task(
            input("Continue investigation", None),
            "ac539c7241644d3da99bf0f06ec93f02",
        )
        .unwrap();
    db.track(TrackingAction::EndBreak { id: pause.id }).unwrap();
    assert!(db.entry(task.id).unwrap().is_running);
    db.track(TrackingAction::EndDay { id: session.id }).unwrap();
    let status = db.tracking_state().unwrap();
    assert!(status.session.is_none() && status.tasks.is_empty() && status.active_break.is_none());
    let day = local_date(session.started_at).unwrap();
    let history = db.work_sessions(&day).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].ended_at, db.entry(task.id).unwrap().end_at);
}

#[test]
fn edits_manual_ranges_and_preserves_end_times_set_by_tracking() {
    use crate::{EntryKind, TrackingAction};

    let mut db = database();
    let (start, _) = day_bounds("2026-06-15").unwrap();
    let mut pause = input("", Some(start));
    pause.kind = EntryKind::Break;
    pause.end_at = Some(start - 1);
    assert!(db.save_entry(None, pause, None).is_err());
    let mut pause = input("", Some(start));
    pause.kind = EntryKind::Break;
    pause.end_at = Some(start + 900_000);
    let pause = db.save_entry(None, pause, None).unwrap();
    assert_eq!(pause.description, "Break");
    assert_eq!(pause.end_at, Some(start + 900_000));
    assert!(!pause.is_running);

    let task = db
        .start_task(input("A task", None), "05e145c236f04be499d1c07278e2c193")
        .unwrap();
    assert!(db.tracking_state().unwrap().session.is_some());
    db.track(TrackingAction::StopTask { id: task.id }).unwrap();
    let stopped = db.entry(task.id).unwrap();
    let mut edit = input("Renamed task", Some(task.activity_at));
    edit.auto_reference = false;
    let edited = db.save_entry(Some(task.id), edit, None).unwrap();
    assert_eq!(edited.end_at, stopped.end_at);

    let mut edit = input("Renamed task", Some(task.activity_at));
    edit.update_end = true;
    let edited = db.save_entry(Some(task.id), edit, None).unwrap();
    assert!(edited.end_at.is_none() && !edited.is_running);
}

#[test]
fn corrects_and_deletes_a_work_session_independently_of_entries() {
    use crate::TrackingAction;

    let mut db = database();
    db.track(TrackingAction::StartDay).unwrap();
    let original = db.tracking_state().unwrap().session.unwrap();
    let start = original.started_at - 60_000;
    db.update_work_session(original.clone(), start, None)
        .unwrap();
    let running = db.tracking_state().unwrap().session.unwrap();
    assert_eq!(running.started_at, start);
    assert!(db.delete_work_session(running.clone()).is_err());
    let task = db
        .start_task(
            input("Keep this task", None),
            "51cb458f97f841fbb7116e3d9c20a807",
        )
        .unwrap();
    db.track(TrackingAction::EndDay { id: running.id }).unwrap();

    let original_date = local_date(start).unwrap();
    let session = db.work_sessions(&original_date).unwrap().pop().unwrap();
    assert!(db.update_work_session(running, start, None).is_err());
    assert!(db
        .update_work_session(session.clone(), start, Some(start - 1))
        .is_err());

    let moved_start = day_bounds("2025-07-15").unwrap().0;
    let moved_end = moved_start + 120_000;
    db.save_review(&original_date, "", ReviewAction::MarkReviewed)
        .unwrap();
    db.save_review("2025-07-15", "", ReviewAction::MarkReviewed)
        .unwrap();
    db.update_work_session(session.clone(), moved_start, Some(moved_end))
        .unwrap();
    assert_eq!(
        db.review(&original_date).unwrap().status,
        ReviewStatus::NeedsReview
    );
    assert_eq!(
        db.review("2025-07-15").unwrap().status,
        ReviewStatus::NeedsReview
    );
    assert!(db.work_sessions(&original_date).unwrap().is_empty());
    assert!(db.delete_work_session(session).is_err());
    let moved = db.work_sessions("2025-07-15").unwrap().pop().unwrap();
    assert_eq!(moved.started_at, moved_start);
    assert_eq!(moved.ended_at, Some(moved_end));

    db.track(TrackingAction::StartDay).unwrap();
    let next = db.tracking_state().unwrap().session.unwrap();
    db.delete_work_session(moved).unwrap();
    assert!(db.work_sessions("2025-07-15").unwrap().is_empty());
    assert_eq!(db.tracking_state().unwrap().session.unwrap(), next);
    assert!(db.entry(task.id).unwrap().end_at.is_some());
}

#[test]
fn workday_totals_subtract_only_the_union_of_breaks_inside_finished_sessions() {
    use crate::{EntryKind, WorkdayTotals};

    let mut db = database();
    let date = "2026-07-15";
    let start = day_bounds(date).unwrap().0;
    let at = |minute: i64| start + minute * 60_000;
    for (begin, end) in [
        (540, Some(720)),
        (690, Some(750)),
        (780, Some(1020)),
        (1050, None),
    ] {
        db.conn
            .execute(
                "INSERT INTO work_sessions (started_at, ended_at) VALUES (?1, ?2)",
                params![at(begin), end.map(at)],
            )
            .unwrap();
    }
    for (begin, end) in [
        (510, Some(555)),
        (550, Some(565)),
        (740, Some(800)),
        (1055, None),
    ] {
        let mut pause = input("Break", Some(at(begin)));
        pause.kind = EntryKind::Break;
        pause.end_at = end.map(at);
        db.save_entry(None, pause, None).unwrap();
    }

    let summary = db.workday_summary(date, at(1050)).unwrap();
    assert_eq!(summary.sessions.len(), 4);
    assert_eq!(
        summary.totals,
        Some(WorkdayTotals {
            worked_ms: 395 * 60_000,
            break_ms: 55 * 60_000,
            completed_breaks: 3,
        })
    );
}

#[test]
fn overnight_work_and_breaks_count_on_the_session_start_date() {
    use crate::{EntryKind, WorkdayTotals};

    let mut db = database();
    let date = "2026-07-15";
    let (_, midnight) = day_bounds(date).unwrap();
    let hour = 3_600_000;
    db.conn
        .execute(
            "INSERT INTO work_sessions (started_at, ended_at) VALUES (?1, ?2)",
            params![midnight - hour, midnight + 2 * hour],
        )
        .unwrap();
    let mut pause = input("Break", Some(midnight + hour));
    pause.kind = EntryKind::Break;
    pause.end_at = Some(midnight + 3 * hour);
    db.save_entry(None, pause, None).unwrap();

    assert_eq!(
        db.workday_summary(date, midnight + 3 * hour)
            .unwrap()
            .totals,
        Some(WorkdayTotals {
            worked_ms: 2 * hour,
            break_ms: hour,
            completed_breaks: 1,
        })
    );
    assert!(db
        .workday_summary("2026-07-16", midnight + 3 * hour)
        .unwrap()
        .totals
        .is_none());
}

#[test]
fn live_workday_totals_advance_and_pause_during_a_running_break() {
    use crate::{EntryKind, WorkdayTotals};

    let mut db = database();
    let date = "2026-07-15";
    let start = day_bounds(date).unwrap().0;
    let at = |minute: i64| start + minute * 60_000;
    db.conn
        .execute(
            "INSERT INTO work_sessions (started_at) VALUES (?1)",
            [at(540)],
        )
        .unwrap();
    let mut pause = input("Recorded break", Some(at(600)));
    pause.kind = EntryKind::Break;
    pause.end_at = Some(at(615));
    db.save_entry(None, pause, None).unwrap();
    db.conn.execute(
        "INSERT INTO work_entries (description, kind, activity_at, created_at, updated_at, is_running)
         VALUES ('Running break', 'break', ?1, ?1, ?1, 1)",
        [at(710)],
    ).unwrap();

    for (minute, worked, on_break, completed) in [
        (530, 0, 0, 0),
        (700, 145, 15, 1),
        (720, 155, 25, 1),
        (735, 155, 40, 1),
    ] {
        assert_eq!(
            db.workday_summary(date, at(minute)).unwrap().totals,
            Some(WorkdayTotals {
                worked_ms: worked * 60_000,
                break_ms: on_break * 60_000,
                completed_breaks: completed,
            })
        );
    }
}

#[test]
fn upgrades_an_active_task_and_manages_two_independent_timers() {
    use crate::TrackingAction;

    let conn = Connection::open_in_memory().unwrap();
    for migration in &MIGRATIONS[..3] {
        conn.execute_batch(migration).unwrap();
    }
    conn.pragma_update(None, "application_id", APPLICATION_ID)
        .unwrap();
    conn.pragma_update(None, "user_version", 3).unwrap();
    let start = Utc::now().timestamp_millis() - 60_000;
    conn.execute(
        "INSERT INTO work_sessions (started_at) VALUES (?1)",
        [start],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO work_entries (description, activity_at, created_at, updated_at, is_running)
         VALUES ('Existing task', ?1, ?1, ?1, 1)",
        [start],
    )
    .unwrap();

    let mut db = Store::initialize(conn).unwrap();
    let original = db.tracking_state().unwrap().tasks.pop().unwrap();
    let second_key = "bb03391bf1424c8da34c416d17bd7cac";
    let second = db
        .start_task(input("Second task", None), second_key)
        .unwrap();
    assert!(db.entry(original.id).unwrap().is_running);
    assert_eq!(db.tracking_state().unwrap().tasks.len(), 2);
    assert_eq!(
        db.start_task(input("Retry second", None), second_key)
            .unwrap()
            .id,
        second.id
    );
    assert!(db
        .start_task(
            input("Third task", None),
            "a373d46ed6884dbfb73cae16bda579d7"
        )
        .is_err());

    db.track(TrackingAction::StopTask { id: original.id })
        .unwrap();
    assert!(db.entry(original.id).unwrap().end_at.is_some());
    assert!(db.entry(second.id).unwrap().is_running);
    let next = db
        .start_task(
            input("New parallel task", None),
            "a373d46ed6884dbfb73cae16bda579d7",
        )
        .unwrap();
    db.track(TrackingAction::StartBreak).unwrap();
    let pause = db.tracking_state().unwrap().active_break.unwrap();
    assert_eq!(db.entry(second.id).unwrap().end_at, Some(pause.activity_at));
    assert_eq!(db.entry(next.id).unwrap().end_at, Some(pause.activity_at));

    let first = db
        .start_task(
            input("Resume work", None),
            "d2c751626eed443b89f2424565f75ec5",
        )
        .unwrap();
    assert_eq!(db.entry(pause.id).unwrap().end_at, Some(first.activity_at));
    let second = db
        .start_task(
            input("Another task", None),
            "f7ac216ad7db4f1ea65c72ed75bf214d",
        )
        .unwrap();
    let session = db.tracking_state().unwrap().session.unwrap();
    db.track(TrackingAction::EndDay { id: session.id }).unwrap();
    assert_eq!(
        db.entry(first.id).unwrap().end_at,
        db.entry(second.id).unwrap().end_at
    );
    assert!(db.entry(first.id).unwrap().end_at.is_some());
    assert!(db.tracking_state().unwrap().tasks.is_empty());
}
