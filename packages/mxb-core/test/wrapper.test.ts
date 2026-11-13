import { afterEach, expect, mock, test } from "bun:test";

let failure = false;
let last: { file: string; args: string[] } | null = null;
mock.module("node:child_process", () => ({
  execFile(file: string, args: string[], _options: unknown, callback: (error: Error | null, stdout: string, stderr: string) => void) {
    last = { file, args };
    if (failure) callback(new Error("exit 2"), "", "fixture failed");
    else callback(null, JSON.stringify(args.includes("folders") ? [{ game: "mxBikes", installDir: "game", userDir: "user", modsDir: "mods", source: "steam" }] : []), "");
  },
}));

const { findInstalls, listMods } = await import("../src/index");

afterEach(() => {
  failure = false;
  last = null;
  delete process.env.MXB_BIN;
});

test("runs mxb mods with JSON and parses its answer", async () => {
  process.env.MXB_BIN = "fake-mxb";
  expect((await findInstalls())[0]?.modsDir).toBe("mods");
  expect(last).toEqual({ file: "fake-mxb", args: ["mods", "folders", "--json"] });
  expect(await listMods("bikes")).toEqual([]);
  expect(last?.args).toEqual(["mods", "list", "--kind", "bikes", "--json"]);
});

test("surfaces CLI errors", async () => {
  process.env.MXB_BIN = "fake-mxb";
  failure = true;
  await expect(findInstalls()).rejects.toThrow("fixture failed");
});
