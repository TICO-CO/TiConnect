CREATE TABLE peer_profiles (
    fingerprint TEXT PRIMARY KEY,
    node_id TEXT,
    display_name TEXT,
    avatar_hash TEXT,
    avatar_mime TEXT,
    avatar BLOB,
    updated_at INTEGER NOT NULL
);

CREATE INDEX peer_profiles_updated ON peer_profiles (updated_at);

ALTER TABLE messages ADD COLUMN read_at INTEGER;

CREATE INDEX messages_conversation_read
    ON messages (conversation_id, sender_id, read_at);
