use std::{fs, io::Write, path::Path, time::Duration};

use rusqlite::{backup::Backup, Connection, OpenFlags};
use tempfile::NamedTempFile;

use crate::{invalid, store::APPLICATION_ID, utc_timestamp, Result, Store};

impl Store {
    pub fn backup(&self, destination: &Path) -> Result<()> {
        self.check_transfer_path(destination)?;
        let temp = temporary_file(destination)?;
        let mut snapshot = Connection::open(temp.path())?;
        copy_database(&self.conn, &mut snapshot)?;
        snapshot.execute_batch("PRAGMA journal_mode = DELETE;")?;
        snapshot.close().map_err(|(_, error)| error)?;
        publish(temp, destination)
    }

    pub fn restore(&mut self, source: &Path) -> Result<()> {
        self.check_transfer_path(source)?;
        let snapshot = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let app_id: i64 = snapshot.pragma_query_value(None, "application_id", |row| row.get(0))?;
        if app_id != APPLICATION_ID {
            return Err(invalid("Choose a backup created by WorkLog."));
        }
        let temp = NamedTempFile::new()?;
        let mut candidate = Connection::open(temp.path())?;
        copy_database(&snapshot, &mut candidate)?;
        candidate.execute_batch("PRAGMA journal_mode = DELETE;")?;
        let candidate = Store::initialize(candidate)?;

        let integrity: String = candidate
            .conn
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if integrity != "ok"
            || candidate
                .conn
                .prepare("PRAGMA foreign_key_check")?
                .exists([])?
        {
            return Err(invalid("The backup failed SQLite's integrity checks."));
        }
        candidate.settings()?.normalize()?;
        candidate.categories()?;
        for table in [
            "work_entries",
            "daily_reviews",
            "reminder_deliveries",
            "work_sessions",
        ] {
            candidate.conn.prepare(&format!("SELECT * FROM {table}"))?;
        }

        // SQLite applies the replacement in one transaction and rolls back an incomplete copy.
        copy_database(&candidate.conn, &mut self.conn)?;
        Ok(())
    }

    pub fn export_csv(&self, destination: &Path) -> Result<()> {
        self.check_transfer_path(destination)?;
        let mut temp = temporary_file(destination)?;
        let mut csv = csv::Writer::from_writer(temp.as_file_mut());
        csv.write_record([
            "id",
            "description",
            "reference_type",
            "reference_number",
            "category",
            "activity_at_utc",
            "created_at_utc",
            "updated_at_utc",
            "entry_kind",
            "end_at_utc",
            "is_running",
        ])?;
        let mut query = self.conn.prepare(
            "SELECT e.id, e.description, e.reference_type, e.reference_number, c.name,
                    e.activity_at, e.created_at, e.updated_at, e.kind, e.end_at, e.is_running
             FROM work_entries e LEFT JOIN categories c ON e.category_id = c.id
             ORDER BY e.activity_at, e.id",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            let text_or_empty = |index| {
                Ok::<_, rusqlite::Error>(row.get::<_, Option<String>>(index)?.unwrap_or_default())
            };
            csv.write_record([
                row.get::<_, i64>(0)?.to_string(),
                row.get(1)?,
                text_or_empty(2)?,
                text_or_empty(3)?,
                text_or_empty(4)?,
                utc_timestamp(row.get(5)?)?.to_rfc3339(),
                utc_timestamp(row.get(6)?)?.to_rfc3339(),
                utc_timestamp(row.get(7)?)?.to_rfc3339(),
                row.get(8)?,
                row.get::<_, Option<i64>>(9)?
                    .map(|time| utc_timestamp(time).map(|time| time.to_rfc3339()))
                    .transpose()?
                    .unwrap_or_default(),
                row.get::<_, bool>(10)?.to_string(),
            ])?;
        }
        csv.flush()?;
        drop(csv);
        publish(temp, destination)
    }

    fn check_transfer_path(&self, target: &Path) -> Result<()> {
        let db = Path::new(self.conn.path().unwrap_or_default());
        if !db.as_os_str().is_empty() {
            let db = fs::canonicalize(db)?;
            let parent = target.parent().ok_or_else(|| invalid("Choose a file."))?;
            let file = target
                .file_name()
                .ok_or_else(|| invalid("Choose a file."))?;
            let target = if target.exists() {
                fs::canonicalize(target)?
            } else {
                fs::canonicalize(parent)?.join(file)
            };
            let live_file = db.as_os_str().to_string_lossy();
            if target == db
                || ["-wal", "-shm", "-journal"]
                    .iter()
                    .any(|suffix| target == Path::new(&format!("{live_file}{suffix}")))
            {
                return Err(invalid("Choose a file outside the live WorkLog database."));
            }
            #[cfg(unix)]
            if target.exists() {
                use std::os::unix::fs::MetadataExt;
                let original = fs::metadata(db)?;
                let selected = fs::metadata(target)?;
                if selected.ino() == original.ino() && selected.dev() == original.dev() {
                    return Err(invalid("Choose a file outside the live WorkLog database."));
                }
            }
        }
        Ok(())
    }
}

fn copy_database(source: &Connection, destination: &mut Connection) -> Result<()> {
    // A busy external backup must fail instead of holding the app's database lock indefinitely.
    let backup = Backup::new(source, destination)?;
    for _ in 0..100 {
        if matches!(backup.step(-1)?, rusqlite::backup::StepResult::Done) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    Err(invalid(
        "The database file is busy. Try the transfer again.",
    ))
}

fn temporary_file(destination: &Path) -> Result<NamedTempFile> {
    let parent = destination
        .parent()
        .ok_or_else(|| invalid("Choose a file."))?;
    Ok(NamedTempFile::new_in(parent)?)
}

fn publish(mut temp: NamedTempFile, destination: &Path) -> Result<()> {
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.persist(destination).map_err(|error| error.error)?;
    Ok(())
}
