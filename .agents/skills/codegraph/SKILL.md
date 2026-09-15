---
name: codegraph
description: >
  Query this repo's local CodeGraph index instead of grep/Read when exploring
  architecture, tracing a flow, or checking blast radius before a change.
  Use when asking how code works, who calls a symbol, what a change affects,
  or when running /codegraph.
license: MIT
metadata:
  author: kim
---

# CodeGraph

This repository is indexed at `.codegraph/` (local SQLite, gitignored). Reach
for the index **before** grep/find or reading files.

## When to use

- "How does X work?" / architecture / a flow from X to Y
- Locate a symbol, route, or handler by name
- Blast radius before editing persist, protocol, router, chat, or royal

## How

- **MCP** (when `codegraph_explore` is available): one call with a natural-language question or symbol/file names. Treat returned source as already Read.
- **CLI** (always): `codegraph explore "<question or symbol names>"`

If the response has a staleness banner, Read **only** the listed files. Do not spawn a file-reading explore subagent for indexed code. Do not run `codegraph init` unless the user asks.

Configs, docs, and files CodeGraph does not index still use Read/Grep.
