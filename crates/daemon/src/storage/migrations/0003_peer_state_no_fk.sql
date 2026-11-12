CREATE TABLE peer_state_new (
    peer_id TEXT PRIMARY KEY,
    last_sent_seq INTEGER NOT NULL DEFAULT 0,
    last_recv_seq INTEGER NOT NULL DEFAULT 0
);

INSERT INTO peer_state_new (peer_id, last_sent_seq, last_recv_seq)
    SELECT peer_id, last_sent_seq, last_recv_seq FROM peer_state;

DROP TABLE peer_state;

ALTER TABLE peer_state_new RENAME TO peer_state;
