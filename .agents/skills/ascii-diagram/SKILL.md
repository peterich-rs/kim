---
name: ascii-diagram
description: >-
  Draw ASCII box-drawing diagrams -- sequence, call-chain, dataflow, layered,
  state -- to reason about and communicate logic paths: request flows, protocol
  handshakes, message pipelines, architecture reuse. Use when explaining how a
  flow works ("how does X reach Y"), tracing a request or message path,
  proposing or reviewing a design, debugging a chain, or writing design docs
  where logic spans multiple components. Diagram first, prose second.
---

# ASCII Diagram

Any logic spanning 2+ hops or 3+ participants gets a diagram instead of prose.
One diagram per flow; complex topics get an overview plus one diagram per flow.

## When to Draw

- Answering "how does X work / X 怎么走到 Y" -- lead the answer with the diagram.
- Proposing a design -- draw current state, then target state with deltas marked.
- Debugging -- draw the path with `✗` at the break point before touching code.
- Reviewing a change -- one diagram of the blast radius when a shared path is touched.
- Design docs, bugfix plans -- embed the target-state diagram; the delta marks
  double as the change summary.

## Workflow

1. **Trace** the real path from code/docs first. Never draw from memory.
2. **Pick one type** (table below) and one frame-wide dimension -- time, OR
   branching, OR layering. Never overlay two frame-wide dimensions (e.g.
   branching inside a sequence diagram). A `→` stage chain inside one branch
   is a stage list, not a second dimension.
3. **Sketch current → target** for changes; mark deltas (see Delta Marking).
4. **Walk every arrow** before emitting: each arrow must map to real code or a
   proposed change. Name participants as they exist in the system
   (`gateway`, `auth`, `relay`), not generically ("server A").
5. **Emit** inside a fenced ```text block, <= 100 columns, legend when more
   than two arrow styles are mixed.

## Diagram Types

| Type | Shape | Use when |
|------|-------|----------|
| Sequence | participant columns, time flows down | request/response, handshake, any back-and-forth between actors |
| Call-chain | top-down spine `\| ▼`, numbered steps | one linear entry path with side annotations |
| Dataflow / pipeline | horizontal `──►`; or top-down fan-out: source on top, `├─ └─` edges to labeled consumers, `→` stage chains per branch | transformation stages, pipelines, one source feeding N consumers |
| Layered | stacked bands | system composition, what reuses what |
| State | `[node] --event--> [node]` | lifecycles: connection, session, token |

Complex topic = layered overview first, then one sequence or dataflow per flow.
Never cram a whole system into one frame.

## Glyph Vocabulary

Semantics are fixed; never repurpose a glyph:

```text
──►   request / sync call        ◄──   response / return value
══►   push / async / fanout      ┄►    optional / conditional path
→     transform stage -- payload changes form (encode / wrap / encrypt)
──┤   blocked / rejected         ✗     failure or timeout point
│ ▼   downward step (spine)      ① ②   ordered steps
├─► └─►  branch                  ◄──►  bidirectional
├─ └─   fan-out edge to a labeled consumer (its pipeline continues below)
←     side annotation at a line  ★     NEW in this design
✂     removed by this design     (existing / 现有)  unchanged by this design
```

Rules:

- More than two arrow styles in one diagram → append a `Legend:` line.
- Payloads ride on the arrow (`──► {jwt, expires_in}`); keep them short --
  link schemas in prose instead of drawing them.
- Every arrow has a direction. Undirected lines are forbidden except lifelines.
- Tree connectors (`├─ └─`, `│`) are structure, not arrows -- they do not
  count toward the legend threshold.
- Inside one continuous processing chain, uniform `→` is fine; switch to
  `──►` when hops cross component boundaries and call-vs-transform matters.
- Pure-ASCII fallback when box glyphs are unwanted: `──►`→`--->`, `══►`→`===>`,
  `┄►`→`--.>`; semantics and legend rules unchanged.
- Diagonals and curves are impossible in monospace; restructure instead.

## Layout Rules

- Always a fenced ```text block -- alignment does not survive outside one.
- <= 100 display columns. Wider → split the diagram, never shrink the glyphs.
- Sequence: participants aligned on the top row, one message per line,
  a `|` lifeline per participant, hops numbered ① ② ③ when order matters.
  Max 5 lanes; more → group into a composite lane or split per flow.
- Call-chain: one `│` spine, one hop per line, annotations to the right of
  `←`, at most one annotation block per 5 spine lines.
- Dataflow: main path on a single horizontal line; branches drop below with
  `├─►`; reuse notes in parentheses at the end of a lane, not inside boxes.
- Pipeline (vertical fan-out): source on top, `├─ └─` edges to labeled
  consumers, a `→` stage chain under each, context in parentheses on the
  branch label, and every branch terminates at a concrete sink -- a field,
  an upload, a store. Never mid-air.
- The diagram must read alone: gist in 5 seconds, every step answerable
  without prose. Detail (fields, error codes, retry policy) goes below it.
- CJK labels are welcome, but alignment counts display columns (one CJK
  char = 2 columns). Verify lane alignment by column math, not by eye.

## Delta Marking

Diagrams accompanying a change mark every element as exactly one of:

- `★` -- added by this design
- unmarked -- existing and unchanged
- `✂` / `✗` -- removed or blocked

Deltas must be scannable at a glance: reviewers diff the two diagrams, not the
prose. Annotation style: `POST /open/v1/device/token  ← ★ 唯一新增`.

## Integration

- `tech-design-to-docs`: target-state diagram goes in the Design section.
- `bugfix-plan-format`: current-state diagram with `✗` at the failure point,
  then target-state with the fix.
- `code-review`: one blast-radius diagram when the change hits a shared path.

## Anti-patterns

- Prose describing a path the reader must mentally reconstruct.
- Glyph drift: `──►` for a push, `══►` for a call, or invented arrows with no
  legend.
- Boxes around everything -- borders are noise; box only groups that need
  visual separation.
- > 5 lanes, > 2 dimensions, or > ~30 lines in one frame -- split it.
- Error paths silently omitted: draw them as `┄►`/`✗` branches or a separate
  alt section, or state explicitly they are out of scope.
- A pipeline branch that ends mid-chain with no named sink -- the reader
  cannot tell where the data lands.

## Checklist Before Emitting

- [ ] One ```text fence, <= 100 display columns
- [ ] One type, one dimension per frame
- [ ] Glyphs match the vocabulary; legend when > 2 arrow styles
- [ ] Steps numbered when order matters
- [ ] Every arrow maps to real code or a proposed change
- [ ] Pipeline branches terminate at a named sink
- [ ] Deltas marked (★ / unmarked / ✂) when the diagram accompanies a change
- [ ] Lanes align counting CJK as 2 columns

## Examples

Canonical examples with commentary, simple through complex:
[examples.md](examples.md).
