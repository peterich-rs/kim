-- Tuple-oracle diff: conversation_inbox vs DISTINCT ON (send_time, message_id).
-- Not the old dual MAX(message_id)/MAX(send_time) GROUP BY.
-- Empty result = zero diff.

\set ON_ERROR_STOP on

WITH last_row AS (
  SELECT DISTINCT ON (
    i.app,
    i.account_a,
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
  ORDER BY
    i.app,
    i.account_a,
    CASE WHEN i.group_id = '' THEN i.account_b ELSE i.group_id END,
    CASE WHEN i.group_id = '' THEN 0 ELSE 1 END,
    i.send_time DESC,
    i.message_id DESC
),
unread AS (
  SELECT
    i.app,
    i.account_a AS account,
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
  GROUP BY 1, 2, 3, 4
),
canon AS (
  SELECT
    l.app, l.account, l.dest, l.kind,
    l.last_message_id, l.last_send_time, l.last_sender,
    c.body AS last_body, c.msg_type AS last_msg_type,
    COALESCE(u.unread, 0) AS unread
  FROM last_row l
  JOIN message_content c ON c.id = l.last_message_id AND c.app = l.app
  LEFT JOIN unread u
    ON u.app = l.app AND u.account = l.account AND u.dest = l.dest AND u.kind = l.kind
),
mat AS (
  SELECT app, account, dest, kind, last_message_id, last_send_time,
         last_sender, last_body, last_msg_type, unread
  FROM conversation_inbox
)
SELECT 'inbox_not_in_canon' AS side, *
FROM (TABLE mat EXCEPT TABLE canon) x
UNION ALL
SELECT 'canon_not_in_inbox' AS side, *
FROM (TABLE canon EXCEPT TABLE mat) y
ORDER BY 1, 2, 3, 4, 5;
