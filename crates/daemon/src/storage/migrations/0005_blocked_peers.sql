CREATE TABLE IF NOT EXISTS blocked_peers (
    fingerprint TEXT PRIMARY KEY,
    node_id TEXT,
    display_name TEXT,
    blocked_at INTEGER NOT NULL
);
