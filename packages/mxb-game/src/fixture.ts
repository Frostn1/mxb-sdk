import type { GameEvent, LapRecord, Snapshot } from "./types.js";
import type { GameSdk } from "./client.js";
import { writeFile } from "node:fs/promises";

export type FixtureEntry =
  | { kind: "state"; offsetMs: number; data: Snapshot }
  | { kind: "event"; offsetMs: number; data: GameEvent };
type FixtureEvent<T = GameEvent> = T extends GameEvent ? Omit<T, "seq" | "at"> : never;

const START = 1_800_000_000_000;
const LAP_MS = [95_400, 94_900, 95_100];

function random(seed = 0x4d5842) {
  return () => {
    seed = (Math.imul(seed, 1_664_525) + 1_013_904_223) >>> 0;
    return seed / 0x1_0000_0000;
  };
}

function point(progress: number): [number, number, number] {
  const a = progress * Math.PI * 2;
  return [Math.cos(a) * 286, 2 + Math.sin(a * 3) * 1.5, Math.sin(a) * 286];
}

function riders(progress: number, completed: number, online: boolean): Snapshot["riders"] {
  return Array.from({ length: 8 }, (_, i) => {
    const local = i === 3;
    const p = (progress + (3 - i) * 0.045 + 1) % 1;
    return {
      raceNum: i + 1,
      name: local ? "You" : `Rider ${i < 3 ? i + 1 : i}`,
      bike: "Example 450",
      category: "MX1",
      active: true,
      local,
      place: i + 1,
      laps: Math.max(0, completed - (i > 5 ? 1 : 0)),
      bestLapMs: completed ? 94_300 + i * 170 : null,
      lastLapMs: completed ? 94_700 + i * 160 : null,
      gapMs: i * 620,
      gapLaps: 0,
      penaltyMs: local && completed >= 2 ? 2_000 : 0,
      status: 0,
      track: online && !local ? null : { progress: p, position: point(p), yawDeg: p * 360 + 90, crashed: false },
    };
  });
}

/** A deterministic, entirely synthetic three-lap race. */
export function generatedFixture(online = false): FixtureEntry[] {
  const rand = random();
  const total = LAP_MS.reduce((a, b) => a + b, 0);
  const boundaries = [0, LAP_MS[0], LAP_MS[0] + LAP_MS[1], total];
  const entries: FixtureEntry[] = [];
  let frame = 0;
  for (let t = 0; t <= total + 1_500; t += 100) {
    const lapIndex = t < boundaries[1] ? 0 : t < boundaries[2] ? 1 : 2;
    const lapStart = boundaries[lapIndex];
    const progress = Math.min(1, (t - lapStart) / LAP_MS[lapIndex]);
    const a = progress * Math.PI * 2;
    const speed = 68 + Math.sin(a * 4) * 22 + rand() * 2;
    const position = point(progress);
    const ms = t - lapStart;
    const completed = boundaries.filter((b) => b > 0 && t >= b).length;
    const lastMs = completed ? LAP_MS[completed - 1] : null;
    const bestMs = completed ? Math.min(...LAP_MS.slice(0, completed)) : null;
    const snapshot: Snapshot = {
      schema: 1, frame: frame++, at: START + t, source: "replay", online,
      session: {
        trackId: "example", trackName: "Example Track", trackLength: 1800,
        eventType: "race", server: online ? "Example Server" : null, phase: "race",
        session: 1, state: t >= total ? 32 : 16, lengthMs: 0, laps: 3,
        clockMs: Math.max(0, total - t), weather: "clear", airTempC: 20,
      },
      local: {
        name: "You", raceNum: 4,
        bike: { id: "example-450", name: "Example 450", category: "MX1", gears: 5, maxRpm: 13_500, limiterRpm: 13_200, shiftRpm: 12_500 },
        onTrack: true, trackTime: t / 1000, lapProgress: progress, speedKmh: speed,
        rpm: Math.min(13_400, Math.round(5_000 + speed * 92)), gear: Math.max(1, Math.min(5, Math.floor(speed / 25) + 1)),
        throttle: Math.max(0, Math.min(1, 0.72 + Math.sin(a * 4) * 0.25)),
        frontBrake: Math.max(0, -Math.sin(a * 4) * 0.45), rearBrake: Math.max(0, -Math.sin(a * 4) * 0.2),
        clutch: 0, steerDeg: Math.sin(a) * 7,
        suspension: { front: 0.35 + Math.sin(a * 7) * 0.12, rear: 0.42 + Math.cos(a * 6) * 0.1, frontM: 0.21, rearM: 0.18 },
        position, velocity: [-Math.sin(a) * speed / 3.6, 0, Math.cos(a) * speed / 3.6], accelG: [Math.sin(a * 4) * 0.3, 1, 0],
        yawDeg: progress * 360 + 90, pitchDeg: Math.sin(a * 3) * 2, rollDeg: Math.sin(a) * 9,
        airborne: Math.sin(a * 6) > 0.985, crashed: t >= 132_000 && t < 135_000,
        engineTempC: 78 + t / total * 8, waterTempC: 70 + t / total * 6, fuelL: 5 - t / total * 1.4,
        lap: { completed, lastMs, bestMs, splitsMs: [0.33, 0.66].filter((x) => progress >= x).map((x) => Math.round(x * LAP_MS[lapIndex])), currentMs: ms },
      },
      riders: riders(progress, completed, online),
    };
    entries.push({ kind: "state", offsetMs: t, data: snapshot });
  }

  let seq = 0;
  const event = (offsetMs: number, data: FixtureEvent) =>
    entries.push({ kind: "event", offsetMs, data: { seq: ++seq, at: START + offsetMs, ...data } as GameEvent });
  event(50, { type: "event_start", local: true, raceNum: 4, track: "Example Track", server: online ? "Example Server" : null });
  event(100, { type: "session_start", local: true, raceNum: 4, phase: "race", session: 1 });
  event(150, { type: "on_track", local: true, raceNum: 4 });
  event(31_000, { type: "holeshot", local: false, raceNum: 1, name: "Rider 1", derived: true });
  event(45_000, { type: "overtake", local: true, raceNum: 4, place: 3, passed: [3] });
  event(120_000, { type: "crash", local: false, raceNum: 2 });
  event(122_000, { type: "recover", local: false, raceNum: 2 });
  event(132_000, { type: "crash", local: true, raceNum: 4 });
  event(135_000, { type: "recover", local: true, raceNum: 4 });
  event(160_000, { type: "communication", local: true, raceNum: 4, what: "penalty", penalty: "cutting", seconds: 2 });
  for (let lap = 0; lap < 3; lap++) {
    const start = boundaries[lap];
    for (let split = 0; split < 3; split++) {
      const timeMs = Math.round(LAP_MS[lap] * (split + 1) / 3);
      event(start + timeMs, { type: "split", local: true, raceNum: 4, split, timeMs, bestDiffMs: lap ? LAP_MS[lap] - LAP_MS[0] : 0 });
    }
    const at = boundaries[lap + 1];
    event(at, { type: "lap", local: true, raceNum: 4, lap: lap + 1, timeMs: LAP_MS[lap], valid: true, best: lap < 2,
      splitsMs: [Math.round(LAP_MS[lap] / 3), Math.round(LAP_MS[lap] * 2 / 3), LAP_MS[lap]], topSpeedKmh: 112 + lap, avgSpeedKmh: 1800 / (LAP_MS[lap] / 3_600_000) / 1000 });
    event(at + 30, { type: "lap", local: false, raceNum: 1, lap: lap + 1, timeMs: 94_300, valid: true, best: lap === 0, overallBest: true });
  }
  event(total + 300, { type: "session_end", local: true, raceNum: 4, session: 1 });
  return entries.sort((a, b) => a.offsetMs - b.offsetMs || (a.kind === "event" ? 1 : -1));
}

/** Record this user's stream to JSON Lines for later replay by the mock. */
export async function recordFixture(sdk: GameSdk, file: string, opts: { seconds: number }): Promise<void> {
  const started = Date.now();
  const lines: string[] = [];
  const stream = sdk.stream({ rate: 20, reconnect: false });
  const add = (kind: "state" | "event", data: Snapshot | GameEvent) => lines.push(JSON.stringify({ kind, offsetMs: Date.now() - started, data }));
  stream.onState((data) => add("state", data));
  stream.onEvent((data) => add("event", data));
  await new Promise((resolve) => setTimeout(resolve, Math.max(0, opts.seconds) * 1000));
  stream.close();
  await writeFile(file, lines.join("\n") + "\n", "utf8");
}

export function lapRecord(event: Extract<GameEvent, { type: "lap" }>): LapRecord | null {
  if (!event.local || !event.splitsMs) return null;
  return { lap: event.lap, timeMs: event.timeMs, valid: event.valid, best: event.best, splitsMs: event.splitsMs,
    topSpeedKmh: event.topSpeedKmh ?? 0, avgSpeedKmh: event.avgSpeedKmh ?? 0, airtimeS: 0.8, crashes: event.lap === 2 ? 1 : 0,
    track: "Example Track", bike: "Example 450", at: event.at };
}
