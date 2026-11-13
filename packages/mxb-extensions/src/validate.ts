import type { ExtManifest } from "./manifest.ts";
import { isPermission } from "./permissions.ts";

export interface ValidationResult {
  valid: boolean;
  errors: string[];
}

function validId(id: string): boolean {
  const parts = id.split(".");
  return parts.length >= 2 && parts.length <= 4 && id.length <= 80 && parts.every((part) =>
    part.length >= 1 && part.length <= 40 && !part.startsWith("-") && /^[a-z0-9-]+$/.test(part));
}

function validVersion(version: string): boolean {
  return /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.test(version);
}

function validOrigin(origin: string): boolean {
  const rest = origin.startsWith("https://") ? origin.slice(8) : "";
  const colon = rest.lastIndexOf(":");
  const host = colon >= 0 ? rest.slice(0, colon) : rest;
  const port = colon >= 0 ? rest.slice(colon + 1) : undefined;
  if (port !== undefined && (!/^\d+$/.test(port) || Number(port) > 65535)) return false;
  return host.length > 0 && host.length <= 253 && host.includes(".") &&
    /^[a-z0-9.-]+$/.test(host) && !host.startsWith(".") && !host.endsWith(".");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** Validate the portable manifest rules. File existence is checked when packing. */
export function validateManifest(value: unknown): ValidationResult {
  const errors: string[] = [];
  if (!isRecord(value)) return { valid: false, errors: ["manifest must be an object"] };
  const manifest = value as Partial<ExtManifest> & Record<string, unknown>;
  if (manifest.manifestVersion !== 1) errors.push("manifestVersion must be 1");
  if (typeof manifest.id !== "string" || !validId(manifest.id)) errors.push("id must be 2 to 4 lower-case dot-separated parts");
  if (typeof manifest.version !== "string" || !validVersion(manifest.version)) errors.push("version must be semantic version x.y.z");
  if (typeof manifest.name !== "string" || manifest.name.trim().length < 1 || [...manifest.name].length > 60) errors.push("name must be 1 to 60 characters");
  if (typeof manifest.author !== "string" || manifest.author.trim().length < 1 || [...manifest.author].length > 60) errors.push("author must be 1 to 60 characters");
  if (typeof manifest.description === "string" && [...manifest.description].length > 400) errors.push("description must be at most 400 characters");
  if (manifest.homepage !== undefined && (typeof manifest.homepage !== "string" || !manifest.homepage.startsWith("https://") || manifest.homepage.length > 300)) errors.push("homepage must be an https:// address");

  const permissions = Array.isArray(manifest.permissions) ? manifest.permissions : [];
  const seen = new Set<unknown>();
  for (const permission of permissions) {
    if (!isPermission(permission)) errors.push(`unknown permission '${String(permission)}'`);
    if (seen.has(permission)) errors.push(`permission '${String(permission)}' is listed twice`);
    seen.add(permission);
  }
  if (manifest.permissions !== undefined && !Array.isArray(manifest.permissions)) errors.push("permissions must be an array");

  const network = Array.isArray(manifest.network) ? manifest.network : [];
  if (network.length && !permissions.includes("network")) errors.push("network hosts need the network permission");
  if (network.length > 10) errors.push("network accepts at most 10 origins");
  for (const origin of network) if (typeof origin !== "string" || !validOrigin(origin)) errors.push(`'${String(origin)}' is not an https://host origin`);
  if (manifest.network !== undefined && !Array.isArray(manifest.network)) errors.push("network must be an array");
  if (manifest.updateUrl !== undefined && (typeof manifest.updateUrl !== "string" || !manifest.updateUrl.startsWith("https://"))) errors.push("updateUrl must be https://");
  if (manifest.contributes !== undefined && !isRecord(manifest.contributes)) errors.push("contributes must be an object");
  return { valid: errors.length === 0, errors };
}

export function assertManifest(value: unknown): asserts value is ExtManifest {
  const result = validateManifest(value);
  if (!result.valid) throw new Error(result.errors.join("; "));
}
