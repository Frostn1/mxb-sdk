# MX Bikes Game SDK

Live MX Bikes data through MXB App, for stream overlays, dashboards, Discord bots, sim-rig and
LED hardware, extensions and AI assistants.

It is **read-only**. Nothing in the SDK can send an input, a command or a setting to the game.
And it is **fair online**: other riders are what the game's own timing screen shows, nothing more
(see [Fairness](#fairness)).

There are three ways in:

| | For | How |
|---|---|---|
| **Local API** | overlays, dashboards, bots, hardware, scripts | HTTP + WebSocket on `127.0.0.1`, with a token. Off until you turn it on. |
| **Extensions** | tabs inside MXB App | `mxb.game` in the sandbox, behind the `game.read` permission |
| **MCP** | Claude Code, Cursor, Claude Desktop and other AI assistants | MCP tools at `POST /mcp` on the local API |

All three read the same data, filtered by the same rules before any of them sees it.

## Versions

The API has an explicit schema version and an SDK semantic version. See
[Game SDK versions](versions.md) for the handshake, compatibility, deprecation policy,
and game-build support.

---

## Quick start

1. **MXB App → Settings → Game SDK → Local API: on.** Copy the token.
2. Start MX Bikes with the app's game integration on.
3. Try it:

   ```bash
   curl -H "Authorization: Bearer YOUR_TOKEN" http://127.0.0.1:47820/v1/state
   ```

4. Or open the sample overlay in a browser or an OBS Browser source:
   `packages/mxb-game/samples/overlay/index.html?token=YOUR_TOKEN`

No game handy? See [Develop without the game](#develop-without-the-game).

---

## How it works

```
MX Bikes ──plugin API──▶ game integration ──shared memory (this user only)──▶ MXB App
                         (copies payloads, 50 Hz)                                     │
                                                       decode · fairness · events ◀───┘
                                                       │
                          ┌────────────────────────────┼──────────────────────────┐
                    extensions (game.read)     local API (127.0.0.1)        MCP (POST /mcp)
```

- The game integration receives PiBoSo's published plugin callbacks and copies
  each payload, byte for byte, into a user-only shared block. Snapshots for state
  that is replaced (telemetry, the timing screen, positions, the entry list), and a numbered ring
  for things that happen (laps, splits, session changes, riders joining), so a lap is never lost
  between two reads. It only copies: no decoding, no network, nothing written to the game.
- **MXB App** reads the block at 50 Hz, decodes it with the published MX Bikes layouts, applies
  the fairness rules, derives events (crashes, overtakes, the holeshot), and serves the result.

---

## Fairness

The game's plugin API hands every plugin a lot about everyone on track. The SDK passes on less.

| | Offline (testing, single player, replays) | Online |
|---|---|---|
| Your own telemetry: speed, rpm, gear, inputs, suspension, position, laps, splits | yes | yes |
| Session: track, type, server name, weather, session phase and clock | yes | yes |
| Other riders: name, number, bike, place, laps, best and last lap, gap, penalties | yes | yes |
| Other riders: lap, overtake, holeshot, join and leave events | yes | yes |
| Other riders: position on track, progress round the lap, whether they are down | yes | **no** |
| Other riders crashing (events) | yes | **no** |
| Your account GUID | no | no |

Online means the game reported a server name for the event. A tool that knew where every rider was
on track would be a radar the game does not give the player, so `Rider.track` is `null` for everyone
but you, and crash events are only yours.

Also:

- **Read-only.** The SDK clients and MCP bridge only send GET requests and the WebSocket
  subscription; they cannot send anything to the game.
- **Off by default.** The local API is closed until you turn it on, listens on `127.0.0.1` only,
  and every request needs the token. **New token** in Settings revokes the old one and closes every
  open stream.
- **Rate limits.** 50 requests a second across the endpoint, at most 16 connections, WebSocket
  frames at most 60 a second, extension tabs at most 20.
- The endpoint checks the `Host` header, so a web page can't reach it through a DNS name pointed at
  `127.0.0.1`.

---

## Local API

Base URL `http://127.0.0.1:47820`. Send the token as `Authorization: Bearer <token>`, or as
`?token=<token>` where a header can't be set (an OBS browser source, a WebSocket in a browser).

| Request | Answer |
|---|---|
| `GET /v1/health` | `{ ok, schema, sdk, app }`. No token needed, no data. |
| `GET /v1/hello` | Version, capabilities, game integration state, and deprecations. Token required. |
| `GET /v1/state` | The current [`Snapshot`](#snapshot) |
| `GET /v1/events?since=<seq>` | `{ events, last }`: events after `seq`, oldest first (the last 500 are kept) |
| `GET /v1/laps` | `{ laps }`: your completed laps this session, with speeds, airtime and crashes |
| `GET /v1/stream` (WebSocket) | Live frames and events, below |
| `POST /mcp` | MCP, below |

Errors are JSON `{ "error": "…" }` with 401 (token), 403 (host), 404, 429 (rate), 503 (connections).

### The stream

```
ws://127.0.0.1:47820/v1/stream?token=<token>&rate=30&topics=state,events
```

- `rate`: frames a second, 1 to 60 (default 20). Events are never throttled.
- `topics`: `state`, `events` or both (default both).

Messages are JSON text:

```json
{ "type": "hello", "schema": 1, "sdk": "1.2.0", "app": "0.21.0", "capabilities": ["state", "events", "laps", "stream", "mcp"], "deprecated": [], "rate": 30 }
{ "type": "state", "data": { …Snapshot… } }
{ "type": "event", "data": { "seq": 41, "type": "lap", "local": true, "lap": 4, "timeMs": 95123, … } }
```

The first `state` arrives right after `hello`. When the game closes the stream stays open and the
snapshot's `source` becomes `"none"`.

### Snapshot

`schema` 1. Full types with comments: [`packages/mxb-game/src/types.ts`](../../packages/mxb-game/src/types.ts).
Times are milliseconds unless named `…S`, speeds km/h, distances metres, angles degrees.

```jsonc
{
  "schema": 1, "frame": 18234, "at": 1763000000000,
  "source": "live",            // "live" | "replay" | "none"
  "online": true,
  "session": {
    "trackId": "club", "trackName": "MXB Club", "trackLength": 1800,
    "eventType": "race",       // testing | race | straight_rhythm
    "server": "Some Server",   // null offline
    "phase": "race",           // waiting | practice | qualifying | warmup | race | unknown
    "session": 6, "state": 16, "lengthMs": 0, "laps": 8, "clockMs": 412000,
    "weather": "clear", "airTempC": 21
  },
  "local": {
    "name": "Frost", "raceNum": 12,
    "bike": { "id": "…", "name": "…", "category": "MX1", "gears": 5, "maxRpm": 13500, "limiterRpm": 13200, "shiftRpm": 12500 },
    "trackTime": 412.3, "lapProgress": 0.62,
    "speedKmh": 61.4, "rpm": 11200, "gear": 3,
    "throttle": 0.92, "frontBrake": 0, "rearBrake": 0, "clutch": 0, "steerDeg": -4.1,
    "suspension": { "front": 0.31, "rear": 0.44, "frontM": 0.21, "rearM": 0.18 },
    "position": [12.1, 3.4, -88.0], "velocity": [ … ], "accelG": [ … ],
    "yawDeg": 81, "pitchDeg": -2, "rollDeg": 11,
    "airborne": false, "crashed": false,
    "engineTempC": 82, "waterTempC": 71, "fuelL": 4.2,
    "lap": { "completed": 3, "currentMs": 41230, "lastMs": 95123, "bestMs": 94870, "splitsMs": [31020] }
  },
  "riders": [
    { "raceNum": 7, "name": "…", "bike": "…", "category": "MX1", "active": true, "local": false,
      "place": 1, "laps": 3, "bestLapMs": 93500, "lastLapMs": 94010, "gapMs": 0, "gapLaps": 0,
      "penaltyMs": 0, "status": 0,
      "track": null }          // withheld online; { progress, position, yawDeg, crashed } offline
  ]
}
```

### Events

Every event has `seq` (resume with `since`), `at` (unix ms) and `type`; most have `raceNum` and
`local` (true when it is about you).

| `type` | Extra fields | Notes |
|---|---|---|
| `event_start` / `event_end` | `track`, `server` | You joined or left an event (a server, or testing) |
| `on_track` / `off_track` | | Your bike went on or off track |
| `session_start` | `phase`, `session` | Practice, qualifying, race… |
| `session_state` / `session_end` | `session`, `state` | `state` raw; `session_end` when it has the complete bit (32) |
| `lap` | `lap`, `timeMs`, `valid`, `best`; yours also `splitsMs`, `topSpeedKmh`, `avgSpeedKmh`; others `overallBest` | |
| `split` | `split`, `timeMs`; yours also `bestDiffMs` | |
| `crash` / `recover` | | Yours always; others offline only |
| `overtake` | `place`, `passed` (race numbers) | In races, from the timing screen |
| `holeshot` | `name`, `derived: true` | The game never passes the holeshot to plugins, so this is who led the timing screen at the race's first split |
| `rider_join` / `rider_leave` | `name` | |
| `communication` | `what` (`retired`, `disqualified`, `penalty`), `penalty` (`jump_start`, `cutting`), `seconds` | |

---

## Extensions

Ask for `game.read`. The install dialog says *Read live game data: your telemetry, your laps and the
timing screen*. `game.state` (is the game running) is separate and unchanged.

```js
const snap = await mxb.game.read();                 // the current Snapshot
const stop = mxb.game.subscribe((s) => draw(s), { rate: 10 });   // up to 20 a second
mxb.game.onEvent((e) => { if (e.type === "lap" && e.local) addLap(e); });
```

Sample: [`extensions/samples/lap-timer`](../../extensions/samples/lap-timer/extension.json), a live lap timer tab.
Load it with Settings → Extensions → Developer mode → Load folder.

---

## AI assistants (MCP)

With the local API on, Settings → Game SDK shows the line to run. For Claude Code:

```bash
claude mcp add --transport http mxb-game http://127.0.0.1:47820/mcp --header "Authorization: Bearer YOUR_TOKEN"
```

Other clients that speak Streamable HTTP take the same URL and header. For clients that only launch
a process, use the stdio bridge in the SDK package:

```json
{ "mcpServers": { "mxb-game": { "command": "mxb-mcp", "env": { "MXB_GAME_TOKEN": "YOUR_TOKEN" } } } }
```

Tools, all read-only:

| Tool | Answers |
|---|---|
| `get_game_state` | The current snapshot |
| `get_last_lap` | "How was my last lap": time, validity, splits, top and average speed, airtime, crashes, against your best and average this session |
| `get_session_laps` | Every lap this session (`limit`) |
| `get_standings` | The timing screen |
| `get_recent_events` | Recent events (`limit`, `type`) |

---

## The TypeScript SDK

[`packages/mxb-game`](../../packages/mxb-game/README.md): typed schemas and a client for browsers and Node 22+.

```ts
import { GameSdk, formatLap } from "@mxb/game";

const mxb = new GameSdk({ token: process.env.MXB_GAME_TOKEN! });
console.log((await mxb.state()).local?.speedKmh);

const stream = mxb.stream({ rate: 30 });
stream.onState((s) => render(s));
stream.onEvent("lap", (e) => e.local && console.log(formatLap(e.timeMs)));
stream.onEvent("overtake", (e) => e.local && console.log(`P${e.place}`));
```

Not on npm yet: build it from the repo (`cd packages/mxb-game && bun run build`) and import
`dist/index.js`.

### Samples

| Sample | |
|---|---|
| [`samples/overlay`](../../packages/mxb-game/samples/overlay/index.html) | A 1920×1080 stream overlay: timing tower, place, speed, gear, rpm, pedals, lap timer and an event ticker. One HTML file, transparent background. Query: `token`, `url`, `rows`, `units=mph`. |
| [`extensions/samples/lap-timer`](../../extensions/samples/lap-timer/extension.json) | A lap timer tab inside MXB App |
| [`samples/node/laps.mjs`](../../packages/mxb-game/samples/node/laps.mjs) | Prints laps, passes and crashes as they happen, and a summary on Ctrl+C |

---

## Develop without the game

A mock server for developing without the game is planned; until then, test against the recorded fixtures in the tests.

---

## Limits

- **Windows only for now.** On Linux and macOS the game runs under Wine or Proton, whose shared
  memory a native app can't open.
- MX Bikes only. GP Bikes and Kart Racing Pro send other layouts; the feed carries them, the app
  doesn't decode them yet.
- `phase` is derived from the game's session index in the order the game itself lists sessions;
  `session` and `state` are passed on raw for anything finer.
- The holeshot is derived (see Events).
