CREATE TABLE peers (
    id TEXT PRIMARY KEY,
    fingerprint TEXT NOT NULL UNIQUE,
    display_name TEXT,
    policy TEXT NOT NULL CHECK (policy IN ('accept', 'mute', 'ignore')),
    trusted_at INTEGER NOT NULL,
    last_seen INTEGER
);

CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    peer_id TEXT NOT NULL REFERENCES peers(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL
);

CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    sender_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    sent_at INTEGER NOT NULL,
    received_at INTEGER NOT NULL,
    kind TEXT NOT NULL,
    target_id TEXT,
    body TEXT,
    attachment_hash TEXT,
    signature BLOB,
    UNIQUE (conversation_id, sender_id, seq)
);

CREATE TABLE outbox (
    message_id TEXT PRIMARY KEY,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_try INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);

CREATE TABLE attachments (
    hash TEXT PRIMARY KEY,
    size INTEGER NOT NULL,
    path TEXT NOT NULL,
    mime TEXT
);

CREATE TABLE peer_state (
    peer_id TEXT PRIMARY KEY REFERENCES peers(id) ON DELETE CASCADE,
    last_sent_seq INTEGER NOT NULL DEFAULT 0,
    last_recv_seq INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX messages_conversation_sent_at ON messages (conversation_id, sent_at);
CREATE INDEX messages_conversation_sender_seq ON messages (conversation_id, sender_id, seq);
