// Save local laps as CSV. Add --telemetry to also save local telemetry at 10 Hz.
// MXB_GAME_TOKEN=<token> node samples/node/session-logger.mjs [folder] [--telemetry]
import { mkdir, appendFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { GameSdk } from "../../dist/index.js";

const token = process.env.MXB_GAME_TOKEN;
if (!token) { console.error("Set MXB_GAME_TOKEN."); process.exit(2); }
const telemetry = process.argv.includes("--telemetry");
const folder = resolve(process.argv.slice(2).find((arg) => !arg.startsWith("--")) ?? "./mxb-sessions");
await mkdir(folder, { recursive: true });
const sdk = new GameSdk({ token, url: process.env.MXB_GAME_URL });
let state = await sdk.state();
let stem = "";
let csv = "";
let telemetryFile = "";
let lastTelemetry = 0;
let beganAt = 0;

const safe = (text) => text.replace(/[^a-z0-9_-]+/gi, "-").replace(/^-|-$/g, "").toLowerCase();
const begin = async () => {
  beganAt = Date.now();
  stem = `${new Date().toISOString().replace(/[:.]/g, "-")}-${safe(state.session?.trackName ?? "session")}`;
  csv = resolve(folder, `${stem}.csv`);
  telemetryFile = resolve(folder, `${stem}-telemetry.jsonl`);
  await writeFile(csv, "lap,timeMs,valid,best,splits,topSpeedKmh,avgSpeedKmh,track,bike,at\n");
  if (telemetry) await writeFile(telemetryFile, "");
};
await begin();

const stream = sdk.stream({ rate: telemetry ? 10 : 2 });
stream.onState((next) => {
  state = next;
  if (telemetry && next.local && Date.now() - lastTelemetry >= 90) {
    lastTelemetry = Date.now();
    void appendFile(telemetryFile, JSON.stringify({ at: next.at, ...next.local }) + "\n");
  }
});
const rotate = () => { if (Date.now() - beganAt > 500) void begin(); };
stream.onEvent("event_start", rotate);
stream.onEvent("session_start", rotate);
stream.onEvent("lap", (lap) => {
  if (!lap.local) return;
  const row = [lap.lap, lap.timeMs, lap.valid, lap.best, `"${(lap.splitsMs ?? []).join(";")}"`, lap.topSpeedKmh ?? "", lap.avgSpeedKmh ?? "",
    `"${state.session?.trackName ?? ""}"`, `"${state.local?.bike.name ?? ""}"`, lap.at].join(",");
  void appendFile(csv, row + "\n");
});
stream.onEvent("session_end", async () => {
  state = await sdk.state();
  const result = {
    session: { track: state.session?.trackName ?? null, type: state.session?.eventType ?? null, phase: state.session?.phase ?? null, server: state.session?.server ?? null },
    results: state.riders.map(({ place, raceNum, name, bike, category, laps, bestLapMs, lastLapMs, gapMs, gapLaps, penaltyMs }) =>
      ({ place, raceNum, name, bike, category, laps, bestLapMs, lastLapMs, gapMs, gapLaps, penaltyMs })),
  };
  await writeFile(resolve(folder, `${stem}-results.json`), JSON.stringify(result, null, 2) + "\n");
});
console.log(`Writing sessions to ${folder}`);
process.on("SIGINT", () => { stream.close(); process.exit(0); });
