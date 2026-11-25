ALTER TABLE messages ADD COLUMN reply_to TEXT;

CREATE INDEX messages_reply_to ON messages (reply_to);
