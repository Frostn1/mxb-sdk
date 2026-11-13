import { readFile, readdir, stat } from "node:fs/promises";
import { dirname, join, parse } from "node:path";

export type PluginCategory = "hud" | "graphics" | "physics" | "fixes" | "timing" | "other";
export type RacingMode = "off" | "stay" | "always-on";
export interface ManifestSetting { file: string; label?: string; where?: "plugins" | "documents" }
export interface ManifestKey { key: string; label: string }
export interface ManifestConflict { with: string; reason: string }
export interface ManifestUpdates { github?: string }

/** Optional `<name>.mxbplugin.json` metadata beside a game plugin. */
export interface PluginManifest {
  name?: string;
  version?: string;
  author?: string;
  description?: string;
  category?: PluginCategory;
  homepage?: string;
  gameVersion?: string;
  files?: string[];
  settings?: ManifestSetting[];
  keys?: ManifestKey[];
  racing?: RacingMode;
  conflicts?: ManifestConflict[];
  updates?: ManifestUpdates;
}

export interface GamePlugin {
  id: string;
  file: string;
  enabled: boolean;
  manifest: PluginManifest | null;
  files: string[];
}

export const STOCK_FILES = ["proxy64.dlo", "proxy_udp64.dlo", "xinput64.dli"] as const;
export const MAX_MANIFEST = 64 * 1024;

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function text(value: unknown, maximum: number): value is string {
  return typeof value === "string" && [...value].length <= maximum && !/[\u0000-\u001f\u007f]/.test(value);
}
function relative(value: string): boolean {
  const path = value.replace(/\\/g, "/").replace(/\/$/, "");
  return Boolean(path) && !path.startsWith("/") && !/^[A-Za-z]:/.test(path) && !path.split("/").some((part: string) => !part || part === "." || part === "..");
}
function strings(value: unknown, maximum: number): value is string[] {
  return Array.isArray(value) && value.length <= maximum && value.every((item) => typeof item === "string");
}

/** Validate a parsed plugin manifest. Unknown fields are ignored. */
export function parseManifest(value: unknown): PluginManifest | null {
  if (!record(value)) return null;
  if (!text(value.name, 60) && value.name !== undefined) return null;
  if (!text(value.author, 60) && value.author !== undefined) return null;
  if (!text(value.description, 400) && value.description !== undefined) return null;
  if (!text(value.version, 32) && value.version !== undefined) return null;
  if (!text(value.gameVersion, 16) && value.gameVersion !== undefined) return null;
  const categories = ["hud", "graphics", "physics", "fixes", "timing", "other"];
  if (value.category !== undefined && !categories.includes(String(value.category))) return null;
  if (value.homepage !== undefined && (!text(value.homepage, 300) || !value.homepage.startsWith("https://"))) return null;
  if (value.files !== undefined && (!strings(value.files, 64) || !value.files.every(relative))) return null;
  if (value.settings !== undefined && (!Array.isArray(value.settings) || value.settings.length > 16 || !value.settings.every((item) =>
    record(item) && typeof item.file === "string" && relative(item.file) &&
    (item.label === undefined || text(item.label, 40)) &&
    (item.where === undefined || item.where === "plugins" || item.where === "documents")))) return null;
  if (value.keys !== undefined && (!Array.isArray(value.keys) || value.keys.length > 16 || !value.keys.every((item) =>
    record(item) && text(item.key, 24) && text(item.label, 80)))) return null;
  if (value.conflicts !== undefined && (!Array.isArray(value.conflicts) || value.conflicts.length > 16 || !value.conflicts.every((item) =>
    record(item) && text(item.with, 80) && text(item.reason, 200)))) return null;
  if (value.racing !== undefined && !["off", "stay", "always-on"].includes(String(value.racing))) return null;
  if (value.updates !== undefined && (!record(value.updates) || (value.updates.github !== undefined &&
    (typeof value.updates.github !== "string" || !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value.updates.github))))) return null;
  return value as PluginManifest;
}

/** Read a manifest. Missing, oversized, malformed, or invalid files return `null`. */
export async function readManifest(path: string): Promise<PluginManifest | null> {
  try {
    if ((await stat(path)).size > MAX_MANIFEST) return null;
    return parseManifest(JSON.parse(await readFile(path, "utf8")));
  } catch { return null; }
}

/** Return `<stem>.mxbplugin.json` for a `.dlo` filename. */
export function manifestName(dloName: string): string {
  return `${parse(dloName).name}.mxbplugin.json`;
}

async function exists(path: string): Promise<boolean> {
  try { await stat(path); return true; } catch { return false; }
}
async function filesFor(root: string, dlo: string, manifest: PluginManifest | null): Promise<string[]> {
  const stem = parse(dlo).name;
  const candidates = [dlo, manifestName(dlo), `${stem}_data`, stem, `${stem}.ini`, `${stem}.log`, ...(manifest?.files ?? [])];
  const files: string[] = [];
  for (const name of candidates) if (relative(name) && !STOCK_FILES.some((stock) => stock.toLowerCase() === name.toLowerCase()) && await exists(join(root, name))) files.push(name.replace(/\\/g, "/"));
  return [...new Set(files)].sort();
}
async function scan(root: string, enabled: boolean): Promise<GamePlugin[]> {
  let entries: Awaited<ReturnType<typeof readdir>>;
  try { entries = await readdir(root, { withFileTypes: true }); } catch { return []; }
  const plugins: GamePlugin[] = [];
  for (const entry of entries) {
    if (!entry.isFile() || !entry.name.toLowerCase().endsWith(".dlo") || STOCK_FILES.some((stock) => stock.toLowerCase() === entry.name.toLowerCase())) continue;
    const id = parse(entry.name).name.toLowerCase();
    const manifest = await readManifest(join(root, manifestName(entry.name)));
    plugins.push({ id, file: join(root, entry.name), enabled, manifest, files: await filesFor(root, entry.name, manifest) });
  }
  return plugins;
}

/** List active and disabled `.dlo` plugins using MXB App's disabled-folder convention. */
export async function listPlugins(pluginsFolder: string): Promise<GamePlugin[]> {
  const game = dirname(pluginsFolder);
  const plugins = [...await scan(pluginsFolder, true), ...await scan(join(game, "mxbapp_disabled", "plugins"), false)];
  return plugins.sort((a, b) => a.id.localeCompare(b.id) || Number(b.enabled) - Number(a.enabled));
}

export const readPluginManifest = readManifest;
