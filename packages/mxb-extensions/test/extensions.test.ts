import { afterEach, expect, test } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { keygen, pack, validateManifest, verify } from "../src/index";

let temp = "";
afterEach(async () => { if (temp) await rm(temp, { recursive: true, force: true }); temp = ""; });

const good = {
  manifestVersion: 1,
  id: "test.laps",
  name: "Laps",
  version: "1.0.0",
  author: "Test",
  permissions: ["game.read"],
};

test("validates good and bad manifests", () => {
  expect(validateManifest(good)).toEqual({ valid: true, errors: [] });
  expect(validateManifest({ ...good, id: "Bad" }).valid).toBe(false);
});

test("rejects an unknown permission", () => {
  const result = validateManifest({ ...good, permissions: ["game.everything"] });
  expect(result.valid).toBe(false);
  expect(result.errors.join(" ")).toContain("unknown permission");
});

test("rejects the removed game actions permission clearly", () => {
  const result = validateManifest({ ...good, permissions: ["game.actions"] });
  expect(result.errors).toContain("game.actions is not available: the SDK is read-only");
});

test("rejects presence combined with network", () => {
  const result = validateManifest({ ...good, permissions: ["friends.presence", "network"] });
  expect(result.errors).toContain(
    "friends.presence cannot be combined with network: it would let other players' presence leave this PC",
  );
});

test("packs and verifies a signed package", async () => {
  temp = await mkdtemp(join(import.meta.dir, ".tmp-ext-"));
  const folder = join(temp, "extension");
  await mkdir(folder);
  await writeFile(join(folder, "extension.json"), JSON.stringify(good));
  await writeFile(join(folder, "view.html"), "<p>laps</p>");
  const key = join(temp, "author.pem");
  const out = join(temp, "laps.mxbext");
  keygen(key);
  pack(folder, { key, out });
  const result = verify(out);
  expect(result.valid).toBe(true);
  expect(result.signed).toBe(true);
  expect(result.manifest.id).toBe("test.laps");
});
