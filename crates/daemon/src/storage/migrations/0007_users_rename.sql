ALTER TABLE peers RENAME TO users;
ALTER TABLE peer_state RENAME TO user_state;
ALTER TABLE blocked_peers RENAME TO blocked_users;
ALTER TABLE peer_profiles RENAME TO user_profiles;

ALTER TABLE user_state RENAME COLUMN peer_id TO user_id;
ALTER TABLE conversations RENAME COLUMN peer_id TO user_id;
ALTER TABLE transfer_partials RENAME COLUMN peer_id TO user_id;
