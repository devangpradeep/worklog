DROP INDEX one_running_entry;
CREATE INDEX running_entries ON work_entries (activity_at, id) WHERE is_running = 1;

-- A break uses both slots so it stays exclusive with running tasks.
CREATE TRIGGER limit_running_entries_insert
BEFORE INSERT ON work_entries
WHEN NEW.is_running = 1
    AND (SELECT COALESCE(SUM(CASE kind WHEN 'break' THEN 2 ELSE 1 END), 0)
         FROM work_entries WHERE is_running = 1)
        + CASE NEW.kind WHEN 'break' THEN 2 ELSE 1 END > 2
BEGIN
    SELECT RAISE(ABORT, 'Stop a running task or end the break before changing this timer.');
END;

CREATE TRIGGER limit_running_entries_update
BEFORE UPDATE OF is_running, kind ON work_entries
WHEN NEW.is_running = 1
    AND (SELECT COALESCE(SUM(CASE kind WHEN 'break' THEN 2 ELSE 1 END), 0)
         FROM work_entries WHERE is_running = 1 AND id <> OLD.id)
        + CASE NEW.kind WHEN 'break' THEN 2 ELSE 1 END > 2
BEGIN
    SELECT RAISE(ABORT, 'Stop a running task or end the break before changing this timer.');
END;
