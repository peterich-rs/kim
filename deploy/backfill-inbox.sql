-- Inbox materialization backfill.
-- Requires Phase 1 lock coverage (legacy/pending/mark_read use the same
-- pg_advisory_xact_lock(hashtext(app), hashtext(account)) key, including sender).
-- Re-runnable. Absolute SET on conflict — never DO NOTHING.
-- Oracle is tuple order (send_time DESC, message_id DESC), not the old dual MAX.
-- Terminate the generator with \gexec only (no semicolon) so psql does not
-- print-and-replay the format query.

\set ON_ERROR_STOP on
SET lock_timeout = '5s';
SET statement_timeout = '10min';

SELECT format(
$f$
BEGIN;
SELECT pg_advisory_xact_lock(hashtext(%L), hashtext(%L));
WITH last_row AS (
  SELECT DISTINCT ON (
    CASE WHEN i.group_id = '' THEN i.account_b ELSE i.group_id END,
    CASE WHEN i.group_id = '' THEN 0 ELSE 1 END
  )
    i.app,
    i.account_a AS account,
    CASE WHEN i.group_id = '' THEN i.account_b ELSE i.group_id END AS dest,
    (CASE WHEN i.group_id = '' THEN 0 ELSE 1 END)::smallint AS kind,
    i.message_id AS last_message_id,
    i.send_time AS last_send_time,
    CASE WHEN i.direction = 1 THEN i.account_a ELSE i.account_b END AS last_sender
  FROM message_index i
  WHERE i.app = %L AND i.account_a = %L
  ORDER BY
    CASE WHEN i.group_id = '' THEN i.account_b ELSE i.group_id END,
    CASE WHEN i.group_id = '' THEN 0 ELSE 1 END,
    i.send_time DESC,
    i.message_id DESC
),
unread AS (
  SELECT
    CASE WHEN i.group_id = '' THEN i.account_b ELSE i.group_id END AS dest,
    (CASE WHEN i.group_id = '' THEN 0 ELSE 1 END)::smallint AS kind,
    count(*) FILTER (
      WHERE i.direction = 0
        AND i.message_id > COALESCE(r.last_read_id, 0)
    )::int AS unread
  FROM message_index i
  LEFT JOIN conversation_reads r
    ON r.app = i.app AND r.account = i.account_a
   AND (
        (i.group_id = '' AND r.peer = i.account_b AND r.group_id = '')
        OR (i.group_id <> '' AND r.peer = '' AND r.group_id = i.group_id)
      )
  WHERE i.app = %L AND i.account_a = %L
  GROUP BY 1, 2
)
INSERT INTO conversation_inbox (
  app, account, dest, kind, last_message_id, last_send_time,
  last_sender, last_body, last_msg_type, unread
)
SELECT
  l.app, l.account, l.dest, l.kind,
  l.last_message_id, l.last_send_time, l.last_sender,
  c.body, c.msg_type, COALESCE(u.unread, 0)
FROM last_row l
JOIN message_content c ON c.id = l.last_message_id AND c.app = l.app
LEFT JOIN unread u ON u.dest = l.dest AND u.kind = l.kind
ON CONFLICT (app, account, dest, kind) DO UPDATE SET
  last_message_id = EXCLUDED.last_message_id,
  last_send_time  = EXCLUDED.last_send_time,
  last_sender     = EXCLUDED.last_sender,
  last_body       = EXCLUDED.last_body,
  last_msg_type   = EXCLUDED.last_msg_type,
  unread          = EXCLUDED.unread;
COMMIT;
$f$, s.app, s.account, s.app, s.account, s.app, s.account
)
FROM (
  SELECT DISTINCT app, account_a AS account
  FROM message_index
  ORDER BY 1, 2
) s
\gexec
