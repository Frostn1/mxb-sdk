// Reload on save: for mod makers. Watches your MX Bikes mods folder and, when you save a file,
// asks the running game to pick it up: only the list that changed (a track, a bike, gear, a
// paint), and a bike's gfx.cfg straight onto the bike. No restart, no menu dance.
//
//   npm run build                                  (in packages/mxb-game, once)
//   MXB_GAME_TOKEN=<token> node samples/node/reload-on-save.mjs "C:\Users\you\Documents\PiBoSo\MX Bikes\mods"
//
// Needs "Allow actions" on in MXB App -> Settings -> Game SDK and a compatible integration.
// A content screen open in the game (the bike or track chooser) makes the refresh wait until
// you leave it; the script says so.
import { statSync, watch } from "node:fs";
import { join, sep } from "node:path";
import { GameSdk } from "../../dist/index.js";

const token = process.env.MXB_GAME_TOKEN;
const mods = process.argv[2];
if (!token || !mods) {
  console.error('usage: MXB_GAME_TOKEN=<token> node reload-on-save.mjs "<MX Bikes mods folder>"');
  process.exit(2);
}
const mxb = new GameSdk({ token, url: process.env.MXB_GAME_URL });

// What a saved file asks for. `bike` only for a gfx.cfg, which has its own, lighter action.
function classify(rel) {
  const parts = rel.split(/[\\/]/).filter(Boolean);
  const top = (parts[0] || "").toLowerCase();
  const file = (parts[parts.length - 1] || "").toLowerCase();
  if (file.endsWith(".pnt")) return { lists: "paints" };
  if (top === "tracks") return { lists: "tracks" };
  if (top === "bikes" && file === "gfx.cfg" && parts.length >= 3) return { gfx: parts[1] };
  if (top === "bikes") return { lists: "bikes" };
  if (top === "rider") return { lists: "gear" };
  return { lists: "all" };
}

let pending = new Set();
let gfx = new Set();
let timer = null;

async function flush() {
  timer = null;
  const lists = [...pending];
  const bikes = [...gfx];
  pending = new Set();
  gfx = new Set();
  try {
    for (const bike of bikes) {
      const { id } = await mxb.reloadBikeGfx(bike);
      report(`gfx.cfg of ${bike}`, await mxb.waitForAction(id));
    }
    if (lists.length) {
      // Two different lists at once: one full reload is cheaper than two partial ones.
      const what = lists.length === 1 ? lists[0] : "all";
      const { id } = await mxb.refresh(what);
      report(`${what} refresh`, await mxb.waitForAction(id, 120_000));
    }
  } catch (e) {
    console.error(`  ${e.message}`);
  }
}

function report(what, s) {
  const extra = s.detail ? ` (${s.detail})` : "";
  console.log(`  ${what}: ${s.state}${extra}`);
}

const stream = mxb.stream({ state: false });
stream.onEvent("action", (e) => {
  if (e.state === "waiting") console.log(`  waiting: ${e.detail}`);
});

console.log(`Watching ${mods}. Save a file to see it in the game.`);
watch(mods, { recursive: true }, (_kind, file) => {
  if (!file || file.includes(`${sep}.`) || file.endsWith("~") || file.endsWith(".tmp")) return;
  // A folder "changes" whenever a file in it does; the file's own event is the one that counts.
  try {
    if (statSync(join(mods, String(file))).isDirectory()) return;
  } catch {
    // Deleted: still a change to the list it was in.
  }
  const c = classify(String(file));
  console.log(`changed: ${file}`);
  if (c.gfx) gfx.add(c.gfx);
  else pending.add(c.lists);
  if (pending.has("all")) pending = new Set(["all"]);
  clearTimeout(timer);
  // Editors save in bursts (temp file, rename, touch): wait for the burst to end.
  timer = setTimeout(flush, 800);
});
