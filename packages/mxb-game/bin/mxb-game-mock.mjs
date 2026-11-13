#!/usr/bin/env node
import { startMockServer } from "../dist/index.js";

const args = process.argv.slice(2);
const value = (name) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
if (args.includes("--help")) {
  console.log("mxb-game-mock [--port 47820] [--token TOKEN] [--online] [--speed 1] [--no-loop] [--fixture FILE]");
  process.exit(0);
}
const server = await startMockServer({
  port: value("--port") === undefined ? undefined : Number(value("--port")),
  token: value("--token"), online: args.includes("--online"),
  speed: value("--speed") === undefined ? undefined : Number(value("--speed")),
  loop: !args.includes("--no-loop"), fixture: value("--fixture"),
});
console.log(`MXB Game mock: ${server.url}`);
console.log(`MXB_GAME_TOKEN=${server.token}`);
const stop = async () => { await server.close(); process.exit(0); };
process.on("SIGINT", stop); process.on("SIGTERM", stop);
