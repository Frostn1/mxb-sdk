import { afterEach, expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { listPlugins, manifestName, parseManifest, readManifest } from "../src/index";

let temp = "";
afterEach(async () => { if (temp) await rm(temp, { recursive: true, force: true }); temp = ""; });

test("reads the documented manifest name and fields", async () => {
  temp = await mkdtemp(join(import.meta.dir, ".tmp-plugins-"));
  const path = join(temp, "alpha.mxbplugin.json");
  await writeFile(path, JSON.stringify({ name: "Alpha", category: "hud", files: ["alpha_data/"], racing: "stay" }));
  expect(manifestName("alpha.dlo")).toBe("alpha.mxbplugin.json");
  expect((await readManifest(path))?.name).toBe("Alpha");
  expect(parseManifest({ files: ["../escape"] })).toBeNull();
});
test("lists active and disabled dlo files, never stock shims", async () => {
  temp = await mkdtemp(join(import.meta.dir, ".tmp-plugins-"));
  const active = join(temp, "plugins");
  const disabled = join(temp, "mxbapp_disabled", "plugins");
  await mkdir(active, { recursive: true });
  await mkdir(disabled, { recursive: true });
  await writeFile(join(active, "alpha.dlo"), "binary");
  await writeFile(join(active, "alpha.mxbplugin.json"), JSON.stringify({ name: "Alpha" }));
  await writeFile(join(active, "proxy64.dlo"), "stock");
  await writeFile(join(disabled, "beta.dlo"), "binary");
  const plugins = await listPlugins(active);
  expect(plugins.map((plugin) => [plugin.id, plugin.enabled])).toEqual([["alpha", true], ["beta", false]]);
  expect(plugins[0]?.files).toContain("alpha.mxbplugin.json");
});
