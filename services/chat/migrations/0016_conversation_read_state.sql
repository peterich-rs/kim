CREATE SEQUENCE IF NOT EXISTS conversation_state_version_seq;

ALTER TABLE conversation_inbox
    ADD COLUMN IF NOT EXISTS state_version BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS max_message_id BIGINT NOT NULL DEFAULT 0;

UPDATE conversation_inbox
   SET max_message_id = last_message_id
 WHERE max_message_id = 0
   AND last_message_id > 0;

UPDATE conversation_inbox
   SET state_version = nextval('conversation_state_version_seq')
 WHERE state_version = 0;
