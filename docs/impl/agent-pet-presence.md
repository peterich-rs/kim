# Add Agent Pet Presence to Flutter IM

## Feasibility Assessment

Fully feasible as a Dart-only UI slice. Agent busy state already reaches the chat thread: registered 1:1 via `chat.bot.typing` → `SessionUpdateDto.typing` → `typingProvider` → `KimTypingRow`; desktop Goose events already distinguish `assistant_finished` / `failed` in `AgentRunLoop._promptGoose`. Avatars are a single widget (`KimAvatar`) used in `ChatTitleChrome` and the typing row — both are Agent-session-only hang points, not list cells. Codex pet playback is `drawImageRect` over an 8×9 atlas; no protocol, store, or FFI change is required for P0. Caveat: desktop owner currently does **not** light `typingProvider` locally (docs claim it, `AgentRunLoop` does not) — this slice adds an `AgentRunSink` so the pet has a local running/failed/review signal even before typing is mirrored.

## Current Surface Inventory

- `sdk/mobile/lib/features/session/typing.dart` `TypingNotifier.applyPush` / `peerTypingProvider` — boolean busy per dest; 20s TTL
- `sdk/mobile/lib/features/session/link.dart` (~216) — maps `SessionUpdateDto.typing` into `typingProvider`
- `sdk/mobile/lib/features/chats/chat_page.dart` (~84–98, 149–158, 242–244) — Agent dest detection, typing footer, `ChatTitleChrome` static avatar
- `sdk/mobile/lib/features/chats/chat_chrome.dart` `ChatTitleChrome` (~142–217) — frosted title chip, always `KimAvatar`
- `sdk/mobile/lib/design/kim_typing_bars.dart` `KimTypingRow` — already takes a `Widget avatar`
- `sdk/mobile/lib/design/kim_avatar.dart` — 36/48/72 px circle or squircle; unchanged for humans
- `sdk/mobile/lib/features/agent/mention.dart` `isAgentDest` / `isServerBotAccount` / `profileForChatDest`
- `sdk/mobile/lib/bridge/agent_bridge.dart` `AgentRunLoop._promptGoose` (~143–148) — waits on Goose `listen()` until `assistant_finished` or `failed`; no UI sink
- `sdk/mobile/lib/main.dart` (~88–90) — `AgentRunLoop(bridge, AgentBridge()).start()` with no Riverpod
- `sdk/mobile/lib/features/chats/chats_page.dart` / `design/conversation_tile.dart` — inbox avatars; **out of scope**
- `sdk/mobile/pubspec.yaml` — no `assets:` yet; Flutter 3.47.2

## Design

```text
current
  chat.bot.typing ──► typingProvider ──► KimTypingRow + static KimAvatar
  Goose listen     ──► AgentRunLoop return string only; UI 无 running/failed

target
  chat.bot.typing ──► typingProvider ──┐
  AgentRunSink    ══► runStatus        ├─► presence(dest).phase
  ChatPage open   ──► waveOnce         │         │
  inbound bot msg ──► reviewPulse      ┘         ▼
                                           PetView
                                             │
                                             ▼
                                           AtlasPainter  (pet.json + spritesheet)
                                             │
                                             ▼
                                           ChatTitleChrome / KimTypingRow
```

Legend: `──►` sync/read; `══►` async fan-in from Goose.

```text
[idle] --open--> [wave] --clip done--> [idle]
[idle] --typing|begin--> [running]
[running] --finish--> [review] --clip done--> [idle]
[running] --fail--> [failed] --clip done--> [idle]
[idle] --fail pulse--> [failed] --clip done--> [idle]
```

`waiting` is defined in the pack contract but **not driven in P0** (no stable "awaiting user" signal on the Dart side yet).

### Key design decisions

1. **Pack format = Codex pet, renderer = `dart:ui` atlas** — reuse `pet.json` + spritesheet so later Petdex/hatch-pet packs drop in. Reject `flutter_scene` / Flame for this slice: a 36–72 px header sprite does not need a GPU scene graph, `--enable-flutter-gpu`, or a per-cell `SceneView`. Scene stays a future `PetRenderer` behind the same `PetPhase`.

2. **`PetPhase` is UI presence, not Goose enum and not `typingProvider`** — typing is a boolean; the pet needs one-shot clips (`wave` / `review` / `failed`). Derive phase in Dart. Do not add fields to `AgentProfile` / AgentSpec / FFI.

3. **One `PetView` per open Agent thread, never in `ConversationTile`** — hang points are `ChatTitleChrome` and `KimTypingRow` only. Inbox stays static `KimAvatar`. Avoids N tickers in a reverse list.

4. **Desktop run loop gets an `AgentRunSink`; phones use typing + inbound message pulse** — Goose does not run on iOS/Android. Phones: `running` from `peerTypingProvider`, `review` from a new inbound bot message while `ChatPage` is mounted. Desktop: sink `begin`/`finish` plus the same typing path. Failed clip is desktop-only unless we later persist a failed card.

5. **Default bundled pack, not community galleries** — `assets/pets/default/`. Petdex/OpenPets packs are user-import later and must not ship in the APK. P0 has no pack picker UI.

6. **Clip policy** — `idle` / `running` loop. `wave` / `review` / `failed` play once at pack fps then return to `idle` (or `running` if typing is still true). Priority, high wins: `failed` > `review` > `wave` > `running` > `idle`.

7. **Pixel filtering** — `FilterQuality.none`. Codex cells are 192×208; Impeller bilinear would smear the grid.

### Types

```dart
enum PetPhase { idle, running, review, failed, wave }

class PetClip {
  const PetClip({
    required this.row,
    required this.frames,
    required this.fps,
    required this.loop,
  });
  final int row;
  final int frames;
  final double fps;
  final bool loop;
  Duration get duration => Duration(
        milliseconds: ((frames / fps) * 1000).round(),
      );
}

class PetPack {
  const PetPack({
    required this.id,
    required this.frameWidth,
    required this.frameHeight,
    required this.columns,
    required this.rows,
    required this.clips,
    required this.imageBytes,
  });
  final String id;
  final int frameWidth;  // default 192
  final int frameHeight; // default 208
  final int columns;     // default 8
  final int rows;        // default 9
  final Map<PetPhase, PetClip> clips;
  final Uint8List imageBytes;

  Rect srcRect(PetPhase phase, int frame) { /* col = frame % frames; row from clip */ }
}

class AgentPresence {
  const AgentPresence({required this.phase, this.packId = 'default'});
  final PetPhase phase;
  final String packId;
}
```

Codex row map (defaults if `pet.json` omits `animations`):

| Codex row / name | `PetPhase` | loop |
|---|---|---|
| 0 idle | `idle` | yes |
| 3 waving | `wave` | no |
| 5 failed | `failed` | no |
| 7 running | `running` | yes |
| 8 review | `review` | no |

`running-right` / `running-left` / `jumping` / `waiting` are parsed if present and ignored until a later driver exists.

`pet.json` accepted shape (subset of the Codex/OpenPets contract):

```json
{
  "id": "default",
  "displayName": "KIM",
  "spritesheetPath": "spritesheet.webp",
  "frame": { "width": 192, "height": 208 },
  "animations": {
    "idle":    { "row": 0, "frames": 6, "fps": 8 },
    "waving":  { "row": 3, "frames": 4, "fps": 8 },
    "failed":  { "row": 5, "frames": 8, "fps": 8 },
    "running": { "row": 7, "frames": 6, "fps": 8 },
    "review":  { "row": 8, "frames": 6, "fps": 8 }
  }
}
```

Missing animation → that phase falls back to `idle`. Missing file / decode error → `PetView` renders `KimAvatar` (no blank hole).

### Presence derivation

```dart
abstract class AgentRunSink {
  void begin(String dest);
  void finish(String dest, {required bool failed});
}

// family<AgentPresence, String>
AgentPresence presenceFor(String dest) {
  // 1. failedAt[dest] within clip duration → failed
  // 2. finishedAt[dest] within clip duration → review
  // 3. openedAt[dest] within wave clip, not yet consumed → wave
  // 4. typingProvider.isTyping(dest) || running.contains(dest) → running
  // 5. else idle
}
```

`ChatPage` calls `markOpened(dest)` once in `initState` (post-frame). `reviewPulse(dest)` from `ref.listen(threadMessagesProvider)` when a new non-own, non-sys message arrives on an Agent dest.

### Widget contract

```dart
class PetView extends StatefulWidget {
  const PetView({
    super.key,
    required this.dest,
    this.size = KimAvatarSize.sm,
    this.shape = KimAvatarShape.squircle,
    this.fallbackName = '',
    this.fallbackUrl = '',
  });
}
```

`PetView` watches `agentPresenceProvider(dest)`, loads the pack once, ticks at clip fps, paints with `CustomPainter`. Same outer size as `KimAvatar` so `ChatTitleChrome` / typing row layout does not change.

`ChatTitleChrome` gains an optional `Widget? avatar`. Null keeps today's `KimAvatar`.

### Usage

```dart
final agent = isAgentDest(id) || isServerBotAccount(id);
final avatar = agent
    ? PetView(
        dest: id,
        size: KimAvatarSize.sm,
        fallbackName: liveTitle,
        fallbackUrl: avatarFor(me, social, id),
      )
    : KimAvatar(name: liveTitle, url: avatarFor(me, social, id), size: KimAvatarSize.sm);
```

## Phase 1: Pack parser and atlas geometry

**File: `sdk/mobile/lib/features/agent/pet_pack.dart`** (new)

- `PetPhase`, `PetClip`, `PetPack`
- `PetPack.parseJson(Map json, Uint8List imageBytes)` — Codex defaults, unknown keys ignored
- `PetPack.srcRect(phase, frame)` — `Rect.fromLTWH(col * w, row * h, w, h)`
- `PetPack.loadAsset(AssetBundle bundle, {String id = 'default'})` — reads `assets/pets/$id/pet.json` + spritesheet path

**File: `sdk/mobile/test/agent/pet_pack_test.dart`** (new)

- default grid 8×9 / 192×208
- waving maps to `PetPhase.wave`
- missing `review` falls back to idle clip
- `srcRect(running, 0)` top = `7 * 208`
- frame index wraps with `% frames`

No Flutter GPU. Pure Dart.

## Phase 2: Presence notifier and run sink

**File: `sdk/mobile/lib/features/agent/agent_presence.dart`** (new)

- `AgentRunStatus` holds `running`, `finishedAt`, `failedAt`, `openedAt`
- `AgentRunStatusNotifier` implements `AgentRunSink`
- `markOpened(dest)`, `reviewPulse(dest)` (sets `finishedAt = now` if not running)
- `agentPresenceProvider = Provider.family<AgentPresence, String>` combining status + `peerTypingProvider`
- Clip windows read from a static default duration (800–1200 ms) so Phase 2 tests do not load a pack; Phase 3 replaces with `pack.clips[phase].duration` inside `PetView` only. Presence uses a conservative 1200 ms so clips are not cut short.

**File: `sdk/mobile/test/agent/agent_presence_test.dart`** (new)

- typing true → `running`
- `begin` then `finish(failed: false)` → `review` then `idle` after 1200 ms
- `finish(failed: true)` wins over typing
- `markOpened` → `wave`; second call same dest does not re-trigger until dest is forgotten
- human dest with no typing → `idle` (provider still safe to watch)

**File: `sdk/mobile/lib/bridge/agent_bridge.dart`**

- `AgentRunLoop` takes optional `AgentRunSink? sink`
- `_promptGoose`: `sink?.begin(req.dest)` before `session.prompt`
- on `assistant_finished`: `sink?.finish(dest, failed: false)` then return
- on `failed`: `sink?.finish(dest, failed: true)` then return
- catch in `start()` already submits error result; also `sink?.finish(req.dest, failed: true)`

**File: `sdk/mobile/lib/main.dart`** (~88–90)

- Do **not** start the loop before `ProviderScope`. After `_app` is built, read the notifier from the container, or pass a sink that posts into a top-level `AgentRunStatus` held by a `ProviderContainer` created before `start()`. Preferred: create the `ProviderScope` first, then

```dart
final container = ProviderScope.containerOf(context); // not available yet
```

Use an explicit `ProviderContainer` constructed in `_KimRuntimeHostState`, passed as `parent:` / `ProviderScope(parent: container)`, then:

```dart
final sink = container.read(agentRunStatusProvider.notifier);
unawaited(AgentRunLoop(bridge, AgentBridge(), sink: sink).start());
```

Unregistered dest (`goose` / `agent:id`) uses `req.dest` as the presence key. Registered 1:1 uses the IM dest already on `AgentRunRequestDto` (server `b_…`). No extra mapping.

## Phase 3: Atlas painter and PetView

**File: `sdk/mobile/lib/design/pet_atlas_painter.dart`** (new)

```dart
class PetAtlasPainter extends CustomPainter {
  PetAtlasPainter({
    required this.image,
    required this.src,
    required this.shape,
  });
  final ui.Image image;
  final Rect src;
  final KimAvatarShape shape;

  @override
  void paint(Canvas canvas, Size size) {
    final dst = Offset.zero & size;
    canvas.save();
    final rrect = switch (shape) {
      KimAvatarShape.circle => RRect.fromRectAndRadius(dst, Radius.circular(size.width / 2)),
      KimAvatarShape.squircle => RRect.fromRectAndRadius(dst, Radius.circular(size.width * 0.32)),
    };
    canvas.clipRRect(rrect);
    canvas.drawImageRect(
      image,
      src,
      dst,
      Paint()..filterQuality = FilterQuality.none,
    );
    canvas.restore();
  }

  @override
  bool shouldRepaint(PetAtlasPainter old) =>
      old.image != image || old.src != src || old.shape != shape;
}
```

**File: `sdk/mobile/lib/design/pet_view.dart`** (new)

- `Ticker` / `AnimationController` duration = current clip duration; `repeat` iff `clip.loop`
- On phase change: reset controller, switch clip
- Load pack in `initState` via `PetPack.loadAsset`; cache `ui.Image` on the State
- On load failure: build `KimAvatar` with `fallbackName` / `fallbackUrl`
- `frame = (controller.value * frames).floor().clamp(0, frames - 1)`
- Size from `KimAvatarSize` (sm=36, md=48, lg=72)

**File: `sdk/mobile/test/widgets/pet_view_test.dart`** (new)

- Build with a 16×18 test atlas (8×9 cells of 2×2) injected via a test `PetPack` (add `PetView.debugPack` or a small `PetPackScope`)
- Pump one frame: no exception
- Presence `running` uses a src rect whose `top` matches running row

Do not decode production WebP in widget tests; inject bytes.

## Phase 4: Hang points in the Agent thread

**File: `sdk/mobile/lib/features/chats/chat_chrome.dart`**

- `ChatTitleChrome`: add `Widget? avatar`
- If `avatar != null`, use it in the stack instead of constructing `KimAvatar`
- Presence dot stays `Positioned` on the same stack

**File: `sdk/mobile/lib/features/chats/chat_page.dart`**

- Helper `_agentAvatar(liveTitle)` returning `PetView` when `isAgentDest(id) || isServerBotAccount(id)`, else `KimAvatar`
- Pass into `ChatTitleChrome.avatar` and `KimTypingRow.avatar`
- `initState` / first build: `ref.read(agentRunStatusProvider.notifier).markOpened(widget.id)` only for Agent dests
- `ref.listen(threadMessagesProvider(widget.id), …)`: if Agent dest, new last item is inbound → `reviewPulse(id)`

**File: `sdk/mobile/lib/design/conversation_tile.dart`**

- Unchanged. Inbox keeps `KimAvatar`. Audited so pet tickers do not spawn per row.

**File: `sdk/mobile/lib/features/chats/chats_page.dart`**

- Unchanged (self avatar in the chats app bar).

## Phase 5: Bundled default pack

**File: `sdk/mobile/assets/pets/default/pet.json`** (new)

- Shape in Design; `id: "default"`

**File: `sdk/mobile/assets/pets/default/spritesheet.png`** (new)

- 1536×1872, 8×9, transparent background
- P0 ships a **first-party placeholder**: distinct solid/tint per row so states are visually testable (idle muted, running high-contrast, failed red-tint, review green-tint, wave accent). Not a Petdex character.
- PNG not WebP for decode simplicity in tests and iOS simulator; parser still accepts `.webp` for later hatch-pet imports
- Real brand art is a follow-up; code must not assume more than grid + transparency

**File: `sdk/mobile/pubspec.yaml`**

```yaml
flutter:
  assets:
    - assets/pets/default/
```

Do not glob community packs.

## Phase 6: Verify

- `cd sdk/mobile && dart test test/agent/pet_pack_test.dart test/agent/agent_presence_test.dart`
- `cd sdk/mobile && flutter test test/widgets/pet_view_test.dart test/widgets/kim_typing_bars_test.dart`
- Manual desktop: open Agent 1:1 → wave; send a prompt → running; reply lands → review → idle; kill provider → failed
- Manual phone (registered bot): owner desktop typing push → running on the phone header; inbound reply → review
- Inbox scroll: no pet tickers (devtools / no `PetView` in `ConversationTile`)

## Architectural Notes

- **Semver / protocol**: none. No wire, FFI, `AgentProfile`, or store schema change.
- **Side effects**: one `Ticker` per open Agent `ChatPage` (header). Typing row shares the same `PetView` instance only if both are mounted — two tickers max per thread, acceptable.
- **AgentRunLoop vs Riverpod**: sink is constructor-injected; tests pass a fake sink. `main.dart` uses an explicit `ProviderContainer` so the loop can start without `BuildContext`.
- **Dest identity**: presence key = chat route id (`goose`, `agent:…`, or `b_…`). Same key `peerTypingProvider` already uses.
- **NOT changed**: `kim-agent-host`, Goose event kinds, `chat.bot.typing` semantics, `KimAvatar` for humans, conversation list, web SDK, `flutter_scene`.
- **NOT added**: pack picker, Petdex download, desktop overlay window, visemes, `waiting` driver, Live2D.
- **Future Scene backend**: `PetView` may later swap painter for a single `SceneView` on a dedicated Agent stage. Same `PetPhase`. Do not put `SceneView` in `ChatList` itemBuilder.
- **License**: bundled placeholder is first-party. Importing a Codex community pack later requires a per-pack license field and is out of this slice.

## File Change Summary

- `docs/impl/agent-pet-presence.md` -- this plan
- `docs/impl/README.md` -- index the slice under 待执行 / 客户端
- `sdk/mobile/assets/pets/default/pet.json` -- Codex-subset manifest
- `sdk/mobile/assets/pets/default/spritesheet.png` -- first-party 8×9 placeholder atlas
- `sdk/mobile/lib/bridge/agent_bridge.dart` -- `AgentRunSink` begin/finish around Goose listen
- `sdk/mobile/lib/design/pet_atlas_painter.dart` -- `drawImageRect` + squircle/circle clip
- `sdk/mobile/lib/design/pet_view.dart` -- ticker + pack load + fallback `KimAvatar`
- `sdk/mobile/lib/features/agent/agent_presence.dart` -- run status + `agentPresenceProvider`
- `sdk/mobile/lib/features/agent/pet_pack.dart` -- parser and atlas geometry
- `sdk/mobile/lib/features/chats/chat_chrome.dart` -- optional `avatar` slot on title chip
- `sdk/mobile/lib/features/chats/chat_page.dart` -- `PetView` on Agent dest; open/review pulses
- `sdk/mobile/lib/main.dart` -- `ProviderContainer` + pass sink into `AgentRunLoop`
- `sdk/mobile/pubspec.yaml` -- register `assets/pets/default/`
- `sdk/mobile/test/agent/agent_presence_test.dart` -- phase priority and clip expiry
- `sdk/mobile/test/agent/pet_pack_test.dart` -- JSON defaults and `srcRect`
- `sdk/mobile/test/widgets/pet_view_test.dart` -- paints without GPU / no exception
