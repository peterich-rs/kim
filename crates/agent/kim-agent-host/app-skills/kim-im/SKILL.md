---
name: kim-im
description: Send instant messages from this KIM agent — friends only, one confirmation card per message, never to groups or other agents.
version: 1
---

# Messaging from inside KIM

You are running inside the KIM messenger on the user's own machine. These rules
only hold here; no other agent harness has these tools.

## Who you may message

- `send_message` targets **one KIM user who is already a friend**. Resolve the
  person with `search_contacts` first and use the account id it returns.
- Never send to a group destination, to a local agent profile, or to an id you
  guessed. If `search_contacts` returns nothing, say so and stop.
- You cannot add friends, create groups, or leave the current account.

## Every send is confirmed by the user

- `send_message` does not deliver on its own. The host shows the user a
  confirmation card with the destination and the exact text, and the user may
  allow, deny, or cancel.
- Write the final text before you call the tool. Do not call it to "ask" whether
  a draft is acceptable — put the draft in your reply instead.
- A denied send is a normal outcome. Report it plainly and do not retry the same
  message. `read_clipboard` is confirmed the same way.
- One card per message. Do not fan a single request out into several sends
  without the user asking for that.

## Reading before writing

`search_messages` and `get_conversation_context` read the local message cache
for a thread. Use them to get names, facts, and tone right before you draft.
They are read-only and need no confirmation.

## Reporting back

Tell the user which destination you sent to, in their language. Never claim a
message was delivered when the confirmation was denied or still pending.
