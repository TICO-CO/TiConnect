ALTER TABLE attachments ADD COLUMN name TEXT;
ALTER TABLE attachments ADD COLUMN created_at INTEGER NOT NULL DEFAULT 0;

CREATE TABLE transfer_partials (
    hash TEXT PRIMARY KEY,
    peer_id TEXT NOT NULL,
    size INTEGER NOT NULL,
    bytes_received INTEGER NOT NULL DEFAULT 0,
    temp_path TEXT NOT NULL,
    ranges TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL
);

CREATE INDEX transfer_partials_peer ON transfer_partials (peer_id);

ALTER TABLE messages ADD COLUMN attachment_name TEXT;
ALTER TABLE messages ADD COLUMN attachment_size INTEGER;
ALTER TABLE messages ADD COLUMN attachment_mime TEXT;
