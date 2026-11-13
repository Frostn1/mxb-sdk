// Prints every lap, pass and crash as it happens, and a lap summary when you stop it (Ctrl+C).
//
//   npm run build            (in packages/mxb-game, once)
//   MXB_GAME_TOKEN=<token> node samples/node/laps.mjs
//
// The token is in MXB App -> Settings -> Game SDK, with the local API switched on.
import { GameSdk, formatLap } from "../../dist/index.js";

const token = process.env.MXB_GAME_TOKEN;
if (!token) {
  console.error("Set MXB_GAME_TOKEN (MXB App -> Settings -> Game SDK).");
  process.exit(2);
}
const mxb = new GameSdk({ token, url: process.env.MXB_GAME_URL });
const now = await mxb.state();
console.log(now.session ? `On ${now.session.trackName} (${now.session.phase}).` : "No session yet; waiting.");

const names = new Map(now.riders.map((r) => [r.raceNum, r.name]));
const who = (n) => names.get(n) || `#${n}`;
const stream = mxb.stream({ state: false });

stream.onStatus((up) => console.log(up ? "connected" : "disconnected, retrying"));
stream.onEvent("rider_join", (e) => names.set(e.raceNum, e.name || `#${e.raceNum}`));
stream.onEvent("lap", (e) => {
  if (!e.local) return;
  const flag = e.valid ? (e.best ? "  personal best" : "") : "  invalid";
  console.log(`Lap ${e.lap}: ${formatLap(e.timeMs)}  top ${Math.round(e.topSpeedKmh ?? 0)} km/h${flag}`);
});
stream.onEvent("overtake", (e) => {
  if (e.local) console.log(`Passed ${e.passed.map(who).join(", ")}: now P${e.place}`);
});
stream.onEvent("crash", (e) => console.log(e.local ? "You crashed." : `${who(e.raceNum)} crashed.`));
stream.onEvent("holeshot", (e) => console.log(`Holeshot: ${e.name || who(e.raceNum)}`));

process.on("SIGINT", async () => {
  stream.close();
  const laps = (await mxb.laps()).filter((l) => l.valid);
  if (laps.length) {
    const best = laps.reduce((a, b) => (b.timeMs < a.timeMs ? b : a));
    const avg = laps.reduce((s, l) => s + l.timeMs, 0) / laps.length;
    console.log(`\n${laps.length} valid laps. Best ${formatLap(best.timeMs)} (lap ${best.lap}), average ${formatLap(Math.round(avg))}.`);
  }
  process.exit(0);
});
