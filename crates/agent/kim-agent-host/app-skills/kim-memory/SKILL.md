---
name: kim-memory
description: Keep long-term facts for this KIM agent in MEMORY.md and notes/ inside its private sandbox workspace.
version: 1
---

# Long-term memory in the sandbox workspace

This agent has a private workspace of its own — no other agent profile can read
it. It is the workspace root for `read_file`, `write_file`, and `list_dir`, so
every path below is relative to that root.

```
AGENTS.md     conventions for this workspace (read it, do not rewrite it)
MEMORY.md     durable facts, newest at the bottom
notes/        one file per topic or project
```

## When to read

- Read `MEMORY.md` at the start of a task that depends on earlier sessions
  (preferences, names, decisions, ongoing projects).
- Look in `notes/` with `list_dir` when the task names a topic you may have
  written up before. Do not read every file speculatively.
- The host does not paste memory into the prompt for you. If you did not read
  it, you do not know it.

## When to write

Append to `MEMORY.md` when you learn something durable and small:

- a stable preference or constraint the user restated,
- a decision and its reason,
- an identifier the user will expect you to remember.

Keep entries one or two lines, newest last, and dated when the date matters.
Rewrite an entry instead of contradicting it. Read the file before appending so
you do not drop existing content — `write_file` replaces the whole file.

Put anything longer than a few lines in `notes/<topic>.md` and leave a one-line
pointer in `MEMORY.md`.

## What not to store

Do not write secrets, passwords, tokens, or message bodies the user did not ask
you to keep. Do not treat this workspace as a scratch pad for task output; it is
memory. If `write_file` is not available, say what you would have recorded
instead of pretending it was saved.
