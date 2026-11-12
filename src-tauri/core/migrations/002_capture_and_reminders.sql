ALTER TABLE work_entries ADD COLUMN capture_key TEXT;
CREATE UNIQUE INDEX entries_by_capture_key ON work_entries (capture_key) WHERE capture_key IS NOT NULL;

CREATE TABLE reminder_deliveries (
    review_date TEXT PRIMARY KEY,
    sent_at INTEGER NOT NULL,
    snooze_until INTEGER
);
