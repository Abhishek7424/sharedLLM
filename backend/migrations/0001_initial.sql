-- Migration: Initial schema for Shared Memory Network

-- Roles table
CREATE TABLE IF NOT EXISTS roles (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    max_memory_mb INTEGER NOT NULL DEFAULT 2048,
    can_pull_models INTEGER NOT NULL DEFAULT 0,
    trust_level INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Insert default roles
INSERT OR IGNORE INTO roles (id, name, max_memory_mb, can_pull_models, trust_level, created_at)
VALUES
    ('role-admin', 'admin', 16384, 1, 3, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    ('role-user',  'user',  4096,  1, 2, strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    ('role-guest', 'guest', 1024,  0, 1, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'));

-- Devices table
CREATE TABLE IF NOT EXISTS devices (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    ip TEXT NOT NULL,
    mac TEXT,
    hostname TEXT,
    platform TEXT,
    role_id TEXT REFERENCES roles(id),
    status TEXT NOT NULL DEFAULT 'pending',  -- pending | approved | denied | suspended | offline
    discovery_method TEXT NOT NULL DEFAULT 'manual',  -- mdns | manual
    allocated_memory_mb INTEGER NOT NULL DEFAULT 0,
    last_seen TEXT,
    first_seen TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Allocations table (tracks memory grant history)
CREATE TABLE IF NOT EXISTS allocations (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    memory_mb INTEGER NOT NULL,
    provider TEXT NOT NULL,  -- nvidia | amd | apple | intel | system_ram
    granted_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    revoked_at TEXT
);

-- Settings table (key-value)
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO settings (key, value)
VALUES
    ('trust_local_network', 'false'),
    ('auto_start_ollama', 'true'),
    ('api_port', '8080'),
    ('mdns_enabled', 'true'),
    ('default_role', 'role-guest'),
    ('ollama_host', 'http://127.0.0.1:11434');
