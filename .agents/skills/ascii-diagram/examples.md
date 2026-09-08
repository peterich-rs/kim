# ASCII Diagram -- Canonical Examples

Simple to complex, one example per diagram type. Each entry: the diagram,
then the rules it exercises. CJK counts as 2 display columns.

All scenarios below are FICTIONAL and domain-generic by design: names
(`Auth`, `LinkGateway`, `Relay`, `Core`, `reportDTO`, ...) are invented and
consistent across examples only so the set reads as one coherent demo system.
Learn the shapes and glyph usage, never the business logic.

## 1. Sequence -- device auth + register (4 lanes, ASCII fallback arrows)

```text
   终端 SDK                            Auth(HTTP)         LinkGateway(WSS)     Relay
      |--- POST /open/v1/device/token -->|                  |                  |
      |    {device_id, secret}           | 校验哈希          |                  |
      |<--- {jwt, expires_in} -----------|                  |                  |
      |--- WSS connect ----------------------------------->|                  |
      |--- register(jwt) 第一帧 --------------------------->|-- inner 握手 ----->
      |<-- 注册成功, session_id --------------------------|<-------------------|
      |    (终端现在就是一个在线账号, 收发与普通账号完全一致)
      |<== TaskPush(平台 → 终端) =====(复用现有扇出)==
      |== device.report(终端 → 平台) ===(现有路径)==

   Legend: ---> 请求/同步   ===> 推送/异步(复用现有扇出)   (现有路径) 本次零改动
```

- Four lanes, one message per line, lifelines aligned under participants.
- Payloads ride the arrow (`{device_id, secret}`); long detail stays in prose.
- `--->` vs `===>` = call vs push (pure-ASCII fallback for `──►` / `══►`);
  two arrow styles, so a Legend line is present.
- `(复用现有扇出)` is the reuse/delta convention applied to a sequence
  diagram: reviewers scan for what is new vs reused.
- The parenthetical note row states what register means semantically -- it is
  not a hop, so it gets no arrow.

## 2. Call-chain -- one new openapi on a fully reused path

```text
   接入方进程 (任意语言 / device-daemon / 定时任务)
      │ ① POST /open/v1/device/token {device_id, secret}   ← ★ 唯一新增 openapi
      │    ← { jwt(短时, account=device_xxx, app=demo), expires_in }
      │ ② WSS/LinkGate 连接 + 第一帧 register(jwt)          ← (现有路径,零改动)
      ▼
   gateway ──► relay ◄──► Core          ← device 就是普通 session:
                                           push / presence / ACK /
                                           离线 pull / 幂等 / 限流 全部复用
```

- One `│` spine, ① ② ordering, `▼` lands the flow into the existing subsystem.
- Delta marking at a glance: exactly one `★` (new), everything else annotated
  `(现有路径,零改动)` -- reviewers diff star vs parentheses, not prose.
- `←` side annotations; the response payload sits on its own line under the
  request, keeping hop lines single-purpose.
- The terminal block is a layered note: what the new path plugs into, plus the
  reuse list that justifies "零改动" for the entire tail.

## 3. Dataflow -- push → worker session → result (short loop)

```text
TaskPush ══► 按 (type,tenant) 找/建 worker session ──► runner.execute(args)
                                                        │ 跑到终态
            worker 回 result ◄─────────────────────────┘
```

- Main path on one line, decision inline in the node (`按 (type,tenant)`).
- Arrow semantics are load-bearing: `══►` = async push into the session layer,
  `──►` = sync call into the runner -- two styles, the reader can see which
  hops block.
- The result drops below and loops back with `◄───┘`; no second frame needed.

## 4. Pipeline fan-out -- one camera frame, two consumers

```text
相机 native 帧 (BGR)
        │
        ├─ 快照组包 (libSnapKit.a)
        │     JNI → SnapKitJNIInterface.encodeJpeg  → JPEG 85
        │     → pixelBuffer → Java Base64 → reportDTO.snapshot.images_data
        │
        └─ 封面三图 (imgCache，转码前)
              get_FrameMat 拿 BGR
              JNI → ThumbJNIInterface.encodeJpeg(true) → JPEG 60
              → 替换 reportDTO.cover / banner / poster
              → WebP 转码上传
```

- Vertical fan-out: source on top, `├─ └─` edges to labeled consumers, each
  consumer's processing continues below as a `→` chain. No arrowheads on the
  edges -- the label IS the destination.
- `→` = transform stage: the same payload changes form
  (BGR → JPEG 85 → pixelBuffer → Base64 → DTO field). Uniform `→` is correct
  here because each branch is one continuous chain; a hop crossing a
  component boundary would be `──►`.
- Parenthetical context on branch labels: `(libSnapKit.a)` names the library;
  `(imgCache，转码前)` pins timing -- transcoding happens after these stages.
- Concrete sinks, no mid-air endings: branch 1 lands on the DTO field
  `reportDTO.snapshot.images_data`; branch 2 lands on the upload
  (`WebP 转码上传`).
- One arrow style plus tree connectors → no legend required.
- Quality parameters ride the arrows (`JPEG 85` / `JPEG 60`), so the two
  branches are comparable at a glance.

## 5. Layered -- system overview (mini)

```text
┌ 接入层   终端 SDK / device-daemon / 定时任务
├ 网关层   LinkGateway (WSS/TCP 长连接, 鉴权, 路由)
├ 逻辑层   Relay (会话/扇出) ◄──► Core (账号/关系/鉴权)
└ 存储层   PG / Redis
```

- Bands, not boxes-everywhere: separators only where grouping is the point.
- One line per layer: name + responsibility + the peers it talks to.

## 6. State -- connection lifecycle (mini)

```text
          dial              ok
[offline] ────► [connecting] ────► [online]
   ▲                │                 │
   │                │ timeout ✗       │ close / drop
   └────────────────┴─────────────────┘
```

- Events on arrows; `✗` marks the failure transition.
- The two exits merge into a single return line -- compact, no duplicate
  `[offline]` nodes.

## What these examples collectively demonstrate

- Same glyph vocabulary across all six types; no glyph is repurposed.
- Reuse/delta marks (`现有路径`, `★`) appear wherever a diagram accompanies a
  change, in any type.
- Fictional but internally consistent naming: examples 1, 2, 5 describe the
  same demo platform from different angles -- which is exactly how one real
  system's overview and per-flow diagrams should cohere.
- Detail lives below the diagram, never inside it: fields, error codes, and
  retry policy stay in prose; the frame carries structure and order only.
