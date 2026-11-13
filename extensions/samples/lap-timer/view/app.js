// Lap Timer: the Game SDK through `mxb.game` (needs `game.read`).
//   mxb.game.read()                  the current snapshot
//   mxb.game.subscribe(fn, { rate }) a frame `rate` times a second (at most 20 in a tab)
//   mxb.game.onEvent(fn)             laps, splits, crashes, overtakes...

const $ = (id) => document.getElementById(id);
const fmt = (ms) => {
  if (!ms || ms <= 0) return "–";
  const m = Math.floor(ms / 60000);
  const s = ((ms % 60000) / 1000).toFixed(3).padStart(6, "0");
  return `${m}:${s}`;
};
const laps = [];

function cell(text, cls) {
  const td = document.createElement("td");
  td.textContent = text;
  if (cls) td.className = cls;
  return td;
}

function drawLaps() {
  const valid = laps.filter((l) => l.valid).map((l) => l.timeMs);
  const best = valid.length ? Math.min(...valid) : null;
  const body = $("laps");
  body.replaceChildren();
  for (const l of [...laps].reverse()) {
    const tr = document.createElement("tr");
    if (!l.valid) tr.className = "bad";
    else if (l.timeMs === best) tr.className = "pb";
    const d = l.valid && best && l.timeMs !== best ? `+${((l.timeMs - best) / 1000).toFixed(3)}` : "";
    tr.append(
      cell(String(l.lap), "num"),
      cell(fmt(l.timeMs), "num"),
      cell(d, "num"),
      cell(l.topSpeedKmh ? `${Math.round(l.topSpeedKmh)} km/h` : "–", "num"),
      cell(l.valid ? "" : "invalid"),
    );
    body.append(tr);
  }
}

function draw(s) {
  const l = s && s.local;
  $("idle").hidden = !!l;
  $("live").hidden = !l;
  $("where").textContent = s && s.session ? `${s.session.trackName} · ${s.session.phase}` : "";
  if (!l) return;
  $("lapno").textContent = String(l.lap.completed + 1);
  $("current").textContent = fmt(l.lap.currentMs);
  $("last").textContent = fmt(l.lap.lastMs);
  $("best").textContent = fmt(l.lap.bestMs);
  $("speed").textContent = `${Math.round(l.speedKmh)} km/h`;
  const delta = l.lap.lastMs && l.lap.bestMs ? l.lap.lastMs - l.lap.bestMs : null;
  $("delta").textContent = delta ? `${delta > 0 ? "+" : ""}${(delta / 1000).toFixed(3)}` : "";
  $("delta").className = "num " + (delta > 0 ? "up" : delta < 0 ? "down" : "");
  $("splits").textContent = l.lap.splitsMs.map((ms, i) => `S${i + 1} ${fmt(ms)}`).join("   ");
}

mxb.game.onEvent((e) => {
  if (e.type === "lap" && e.local) {
    laps.push(e);
    drawLaps();
  }
  if (e.type === "event_start") {
    laps.length = 0;
    drawLaps();
  }
});

mxb.game
  .read()
  .then(draw)
  .catch((err) => {
    $("idle").textContent = String(err.message || err);
  });
mxb.game.subscribe(draw, { rate: 10 });
