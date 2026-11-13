// Post local personal bests and the final top ten to a Discord webhook.
// MXB_GAME_TOKEN=<token> DISCORD_WEBHOOK_URL=<url> node samples/node/discord-laps.mjs
import { GameSdk, formatLap } from "../../dist/index.js";

const token = process.env.MXB_GAME_TOKEN;
const webhook = process.env.DISCORD_WEBHOOK_URL;
if (!token || !webhook) { console.error("Set MXB_GAME_TOKEN and DISCORD_WEBHOOK_URL."); process.exit(2); }
const discord = webhook.startsWith("https://discord.com/api/webhooks/") || webhook.startsWith("https://discordapp.com/api/webhooks/");
if (!discord && process.env.MXB_TEST_WEBHOOK_ALLOW_LOCAL !== "1") { console.error("DISCORD_WEBHOOK_URL must be a Discord webhook."); process.exit(2); }
const sdk = new GameSdk({ token, url: process.env.MXB_GAME_URL });

const post = async (content) => {
  const send = () => fetch(webhook, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ content }) });
  let response = await send();
  if (response.status === 429) {
    const body = await response.json().catch(() => ({}));
    await new Promise((resolve) => setTimeout(resolve, Math.max(0, Number(body.retry_after ?? 1)) * 1000));
    response = await send();
  }
  if (!response.ok) console.error(`Discord returned HTTP ${response.status}.`);
};

const stream = sdk.stream({ state: false });
stream.onEvent("lap", (lap) => {
  if (lap.local && lap.valid && lap.best) void post(`Personal best: lap ${lap.lap} — ${formatLap(lap.timeMs)}`);
});
stream.onEvent("session_end", async () => {
  const state = await sdk.state();
  const rows = state.riders.slice(0, 10).map((r) => `${r.place ?? "-"}. ${r.name} — ${formatLap(r.bestLapMs)}`);
  await post(`Final timing — ${state.session?.trackName ?? "session"}\n${rows.join("\n")}`);
});
console.log("Discord lap updates started.");
process.on("SIGINT", () => { stream.close(); process.exit(0); });
