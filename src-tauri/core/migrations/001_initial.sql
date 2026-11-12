CREATE TABLE categories (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL COLLATE NOCASE UNIQUE,
    color TEXT NOT NULL CHECK(length(color) = 7),
    sort_order INTEGER NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0, 1))
);

INSERT INTO categories (name, color, sort_order) VALUES
    ('Development', '#6755d5', 0),
    ('Testing', '#17876f', 1),
    ('Bug Fixing', '#c44865', 2),
    ('Production Support', '#a95119', 3),
    ('Database', '#2676b8', 4),
    ('Deployment', '#5266c3', 5),
    ('Meetings', '#a143a2', 6),
    ('Other', '#697187', 7);

CREATE TABLE work_entries (
    id INTEGER PRIMARY KEY,
    description TEXT NOT NULL CHECK(length(trim(description)) > 0),
    reference_type TEXT,
    reference_number TEXT,
    category_id INTEGER REFERENCES categories(id),
    activity_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    CHECK ((reference_type IS NULL) = (reference_number IS NULL))
);
CREATE INDEX entries_by_activity ON work_entries (activity_at, id);
CREATE INDEX entries_by_category ON work_entries (category_id, activity_at);
CREATE INDEX entries_by_reference ON work_entries (reference_type, reference_number);

CREATE TABLE daily_reviews (
    id INTEGER PRIMARY KEY,
    review_date TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK(status IN ('open', 'reviewed', 'needs_review')),
    notes TEXT NOT NULL DEFAULT '',
    reviewed_at INTEGER
);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
