CREATE TABLE call_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    direction TEXT NOT NULL,
    started_at INTEGER NOT NULL,
    connected_at INTEGER NOT NULL DEFAULT 0,
    ended_at INTEGER NOT NULL,
    duration_secs INTEGER NOT NULL,
    reason TEXT NOT NULL
);

CREATE INDEX call_logs_user_ended ON call_logs (user_id, ended_at);
