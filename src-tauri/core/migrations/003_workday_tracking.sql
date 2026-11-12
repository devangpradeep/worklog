ALTER TABLE work_entries ADD COLUMN kind TEXT NOT NULL DEFAULT 'activity'
    CHECK (kind IN ('activity', 'break'));

ALTER TABLE work_entries ADD COLUMN end_at INTEGER
    CHECK (end_at IS NULL OR end_at >= activity_at);

ALTER TABLE work_entries ADD COLUMN is_running INTEGER NOT NULL DEFAULT 0
    CHECK (is_running IN (0, 1) AND (is_running = 0 OR end_at IS NULL));

CREATE UNIQUE INDEX one_running_entry ON work_entries (is_running) WHERE is_running = 1;

CREATE TABLE work_sessions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at INTEGER NOT NULL,
    ended_at INTEGER CHECK (ended_at IS NULL OR ended_at >= started_at)
);
CREATE INDEX sessions_by_start ON work_sessions (started_at, id);
CREATE UNIQUE INDEX one_open_session ON work_sessions ((1)) WHERE ended_at IS NULL;
