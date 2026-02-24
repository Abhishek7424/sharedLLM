-- Migration: Add UNIQUE constraint on devices.ip to prevent TOCTOU duplicates
-- during concurrent mDNS discovery. Remove any existing duplicates first,
-- keeping the row with the lowest rowid (oldest insert).
DELETE FROM devices
WHERE rowid NOT IN (
    SELECT MIN(rowid) FROM devices GROUP BY ip
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_devices_ip ON devices (ip);
