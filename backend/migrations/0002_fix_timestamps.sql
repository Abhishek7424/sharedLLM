-- Migration: Normalize created_at / first_seen timestamps to ISO 8601 (with T and Z)
-- SQLite's datetime('now') produces "YYYY-MM-DD HH:MM:SS" (space-separated, no timezone).
-- We want RFC3339 "YYYY-MM-DDTHH:MM:SSZ" to match what chrono::Utc::now().to_rfc3339() produces.

UPDATE roles
SET created_at = replace(created_at, ' ', 'T') || 'Z'
WHERE created_at NOT LIKE '%T%';

UPDATE devices
SET created_at  = replace(created_at,  ' ', 'T') || 'Z'
WHERE created_at NOT LIKE '%T%';

UPDATE devices
SET first_seen  = replace(first_seen,  ' ', 'T') || 'Z'
WHERE first_seen NOT LIKE '%T%';

UPDATE devices
SET last_seen   = replace(last_seen,   ' ', 'T') || 'Z'
WHERE last_seen IS NOT NULL AND last_seen NOT LIKE '%T%';

UPDATE allocations
SET granted_at  = replace(granted_at,  ' ', 'T') || 'Z'
WHERE granted_at NOT LIKE '%T%';

UPDATE allocations
SET revoked_at  = replace(revoked_at,  ' ', 'T') || 'Z'
WHERE revoked_at IS NOT NULL AND revoked_at NOT LIKE '%T%';

-- Ensure settings defaults are correct (fix any pre-existing DB rows with wrong values)
INSERT INTO settings (key, value) VALUES ('mdns_enabled', 'true')
    ON CONFLICT(key) DO NOTHING;

INSERT INTO settings (key, value) VALUES ('auto_start_ollama', 'true')
    ON CONFLICT(key) DO NOTHING;
