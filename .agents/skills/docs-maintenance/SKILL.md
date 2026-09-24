---
name: docs-maintenance
description: >
  Keep this repo's docs to two piles: long-term contracts under docs/, and
  slice drafts under docs/impl/ that die when their branch merges. Use when
  adding, moving, or deleting docs, when a branch is ready to merge, or when
  the user asks to clean docs, 清理文档, 合入主干, or docs/impl.
---

# Docs maintenance

The policy text lives in [docs/README.md](../../../docs/README.md) under「文档怎么保持不腐烂」, and the open-slice list lives in [docs/impl/README.md](../../../docs/impl/README.md). Follow those files. Do not restate them into a third copy.

## Classify before writing

| Kind | Where | Keep while |
| --- | --- | --- |
| How the code works now: traits, frames, deploy, a still-open gap | `docs/<topic>.md`, listed in the `docs/README.md` reading order | The behavior or the open gap is still true |
| Slice for work that is not on the default branch yet | `docs/impl/<slice>.md`, one row in `docs/impl/README.md`「未合入」 | That branch is still open |

A draft with no branch, and that does not describe the current system, does not get committed. `research/` stays out of this split.

## Add a slice

1. Put it only in `docs/impl/`, on the branch that implements it.
2. Add one row to「未合入」with the branch name.
3. Do not also add a topic doc that repeats the plan.

## Merge the branch

Do this in the merge, not as a follow-up:

1. If a sentence must still be followed after merge, write it into the existing topic doc. One fact, one file.
2. Delete `docs/impl/<slice>.md`.
3. Remove its「未合入」row. If the shape needs a pointer, add one「已合入」row that links the topic doc, not the deleted slice.
4. Close the matching `G-xx` in `docs/production-gaps.md` when the gap is actually gone.
5. A topic doc that is not in the `docs/README.md` reading list is unfinished. Add it to the list, or it is a stray slice and does not stay at `docs/` root.

Code wins when a doc disagrees with it. Do not paste paid booklet chapters into either pile.
