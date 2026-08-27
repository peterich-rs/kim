---
name: rust-tokio-net
description: >
  Tokio TCP and WebSocket networking for long-lived connections: framing,
  codecs, half-close, backpressure, timeouts, graceful shutdown, and
  read/write task split. Use when writing listeners, dialers, Conn
  implementations, length-prefixed frames, HTTP Upgrade, or debugging
  stuck I/O. Invoke with /rust-tokio-net.
license: MIT
---

# Tokio TCP / WebSocket

Long-lived connection I/O on Tokio. Pair with `rust-async-patterns` for tasks/channels and `rust-skills` `async-*` rules for cancellation and lock hygiene.

This workspace already splits **protocol** (`kim-core` Conn / Channel / ChannelMap) from **wires** (`kim-tcp`, `kim-ws`). Keep that split.

## When to load

- New `TcpListener` / `TcpStream` / WebSocket server or client
- Framing, sticky packets, length prefixes, opcode + payload
- One reader + one writer per connection
- Idle timeout, handshake timeout, max frame size
- Graceful shutdown of a gateway or echo server

## Connection shape

One connection = **one exclusive reader** and **one exclusive writer**. Fan-in writes through a bounded `mpsc`; never share `&mut TcpStream` across tasks.

```text
push / ping ──► bounded mpsc ──► write task ──► socket
socket ──► read task ──► handler (business frames only)
```

- Clone **handles** (Channel, `mpsc::Sender`), not the socket.
- Look up in `ChannelMap` under a read lock, clone the handle, drop the lock, then send. Never hold the map lock across `.await`.
- `OwnedReadHalf` / `OwnedWriteHalf` (TCP) or an equivalent split (WS) after handshake.

## Framing

TCP is a byte stream. Always define a frame:

| Field | Rule |
|-------|------|
| Max size | Reject before allocating; return a typed error and close |
| Length prefix | Decode with `bytes::Buf`; do not `read_exact` a huge length |
| Opcode | Heartbeat (`Ping`/`Pong`) and `Close` stay in the Channel, not the business listener |
| Payload | `bytes::Bytes` (cheap clone, no extra copy on fan-out) |

Do not use `std::io` or `std::net` on the async path. Use `tokio::net` and `tokio::io::{AsyncReadExt, AsyncWriteExt}`.

WebSocket: HTTP Upgrade first (`hyper` + `fastwebsockets` in this repo), then RFC6455 frames. Map WS text/binary/ping/pong/close onto the same `OpCode` as TCP so handlers stay protocol-agnostic.

## Timeouts and idle

```rust
tokio::time::timeout(handshake_timeout, acceptor.accept(&mut conn, handshake_timeout)).await
```

- Handshake: hard timeout; failure closes the socket.
- Idle: read-side timeout or application Ping. Writer owns Pong.
- Per-frame: cap both wall time and byte size.

## Backpressure

- Bounded mailbox (`mpsc` with a small capacity). Slow clients drop or disconnect; do not grow RAM.
- `try_send` / `send_timeout` on the hot push path when a stuck peer must not stall the gateway.
- Never `spawn` an unbounded write task per message.

## Shutdown

1. Stop accepting (`listener` drop or `CancellationToken`).
2. Send `Close` on each Channel; flush.
3. Abort or await read/write tasks with a deadline.
4. Notify `StateListener::disconnect`.

Use `tokio::signal` in binaries (`examples/*`), not library crates.

## Errors

Library crates (`kim-*`): `thiserror` enums (`Io`, `Codec`, `Handshake`, `Closed`). No `.unwrap()` on I/O.

Map `std::io::ErrorKind::UnexpectedEof` / WS close to a clean disconnect, not a panic.

## Tests

- `#[tokio::test]` integration tests bind `127.0.0.1:0`, dial the bound port.
- Cover: echo, max-frame reject, peer reset, concurrent push while reading.
- Do not sleep for synchronization; wait on the event (channel closed, frame received).

## Checklist

- [ ] One reader task, one writer task per conn
- [ ] Map lock not held across await
- [ ] Frame size capped; length decoded before alloc
- [ ] Heartbeats not forwarded to `MessageListener`
- [ ] Bounded write mailbox
- [ ] Handshake and idle timeouts
- [ ] `tracing` spans on accept / disconnect, no secrets in logs
