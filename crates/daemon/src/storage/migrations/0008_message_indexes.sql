CREATE INDEX messages_conversation_sort
    ON messages (
        conversation_id,
        (CASE WHEN sent_at > received_at THEN received_at ELSE sent_at END),
        received_at,
        id
    );

CREATE INDEX messages_conversation_unread
    ON messages (conversation_id, read_at, sender_id);
