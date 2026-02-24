-- Migration: Distributed inference support (llama.cpp RPC)

-- Add RPC fields to devices table
ALTER TABLE devices ADD COLUMN rpc_port INTEGER NOT NULL DEFAULT 8181;
ALTER TABLE devices ADD COLUMN rpc_status TEXT NOT NULL DEFAULT 'offline';
ALTER TABLE devices ADD COLUMN memory_total_mb INTEGER NOT NULL DEFAULT 0;
ALTER TABLE devices ADD COLUMN memory_free_mb INTEGER NOT NULL DEFAULT 0;

-- Inference sessions table
CREATE TABLE IF NOT EXISTS inference_sessions (
    id TEXT PRIMARY KEY,
    model_path TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'starting',  -- starting | running | stopped | error
    devices TEXT NOT NULL DEFAULT '[]',       -- JSON array of device IDs used
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    stopped_at TEXT
);
