import { execFile } from "node:child_process";
export type Game = "mxBikes" | "gpBikes" | "kartRacingPro";
export type Source = "steam" | "manual" | "proton";
export type Kind = "bikes" | "tracks" | "rider" | "paints" | "tyres" | "sounds" | "plugins" | "misc";
export type Packaged = "plain" | "pkz" | "protected";
export interface Install { game: Game; installDir: string; userDir: string; modsDir: string; source: Source }
export interface Mod { kind: Kind; name: string; path: string; sizeBytes: number; files: number; packaged: Packaged }
export interface Identified { kind: Kind | null; layout: string | { needsExtraction: { format: string } }; suggestedName: string }
export interface Write { source: string; destination: string }
export interface Plan { kind: Kind; rule: string; sourceDir: string; modsDir: string; files: Write[] }
export interface Installed { written: string[] }
export interface Problem { code: string; message: string; path: string | null }
export interface Report { problems: Problem[] }
export interface Duplicate { name: string; paths: string[] }
export interface Paint { name: string; path: string; item: string }

async function run<T>(args: string[]): Promise<T> {
  const runtime = globalThis as typeof globalThis & {
    process: { env: Record<string, string | undefined>; execPath: string };
  };
  const binary = runtime.process.env.MXB_BIN || "mxb";
  const scriptRuntime = runtime.process.execPath.toLowerCase().includes("bun") ? "node" : runtime.process.execPath;
  const command = binary.toLowerCase().endsWith(".js") ? scriptRuntime : binary;
  const commandArgs = command === binary ? args : [binary, ...args];
  try {
    const stdout = await new Promise<string>((resolve, reject) => {
      execFile(command, [...commandArgs, "--json"], { windowsHide: true }, (error, out, stderr) => {
        if (error) reject(Object.assign(error, { stderr }));
        else resolve(out);
      });
    });
    return JSON.parse(stdout) as T;
  } catch (error) {
    const detail = error as Error & { stderr?: string };
    throw new Error(detail.stderr?.trim() || detail.message);
  }
}

const mods = <T>(args: string[]) => run<T>(["mods", ...args]);

export const findInstalls = () => mods<Install[]>(["folders"]);
export const listMods = (kind?: Kind) => mods<Mod[]>(kind ? ["list", "--kind", kind] : ["list"]);
export const identify = (path: string) => mods<Identified>(["identify", path]);
export const plan = (source: string) => mods<Plan>(["plan", source]);
export const install = (source: string) => mods<Installed>(["install", source]);
export const uninstall = (path: string) => mods<string>(["uninstall", path]);
export const verify = (path?: string) => mods<Report | Array<[string, Report]>>(path ? ["verify", path] : ["verify"]);
export const duplicates = () => mods<Duplicate[]>(["duplicates"]);
export const paints = (item: string) => mods<Paint[]>(["paints", item]);
