ALTER TABLE peers ADD COLUMN public_key BLOB;

ALTER TABLE messages ADD COLUMN state TEXT;

CREATE UNIQUE INDEX conversations_peer_id ON conversations (peer_id);
CREATE INDEX messages_target_id ON messages (target_id);
CREATE INDEX outbox_next_try ON outbox (next_try);
