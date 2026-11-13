import type { Permission } from "./permissions.ts";

export interface ExtColorSet { [token: string]: string }
export interface ExtTheme {
  id: string;
  label: string;
  dark?: ExtColorSet;
  light?: ExtColorSet;
  radius?: string;
  background?: string;
  buttonStyle?: string;
  fonts?: { body?: string; display?: string };
}
export interface ExtButtonStyle {
  id: string;
  label: string;
  radius?: string;
  uppercase?: boolean;
  letterSpacing?: number;
  weight?: number;
  borderWidth?: number;
  shadow?: "none" | "soft" | "glow";
}
export interface ExtGradient { angle?: number; stops: string[] }
export interface ExtBackground {
  id: string;
  label: string;
  image?: string;
  gradient?: ExtGradient;
  opacity?: number;
  blur?: number;
  position?: "center" | "top" | "bottom";
  size?: "cover" | "contain";
}
export interface ExtFont {
  id: string;
  family: string;
  src: string;
  weight?: string;
  style?: "normal" | "italic";
}
export interface ExtView { id: string; title: string; entry: string }
export interface ExtCommand { id: string; title: string }
export type ExtSettingType = "boolean" | "string" | "number" | "enum";
export interface ExtSetting {
  key: string;
  type: ExtSettingType;
  label: string;
  default?: boolean | string | number;
  options?: string[];
}
export interface ExtMcpTool {
  name: string;
  description: string;
  inputSchema?: Record<string, unknown>;
}
export interface ExtContributes {
  themes?: ExtTheme[];
  buttonStyles?: ExtButtonStyle[];
  backgrounds?: ExtBackground[];
  fonts?: ExtFont[];
  views?: ExtView[];
  main?: string;
  commands?: ExtCommand[];
  settings?: ExtSetting[];
  mcp?: { tools?: ExtMcpTool[] };
}
export interface ExtManifest {
  manifestVersion: 1;
  id: string;
  name: string;
  version: string;
  author: string;
  description?: string;
  homepage?: string;
  minAppVersion?: string;
  permissions?: Permission[];
  network?: string[];
  updateUrl?: string;
  contributes?: ExtContributes;
  screenshots?: string[];
  categories?: string[];
  tags?: string[];
}
export type Trust = "builtin" | "verified" | "signed" | "unsigned";
export interface PendingUpdate { version: string; url: string; newPermissions: string[] }
export interface InstalledExtension {
  id: string;
  manifest: ExtManifest;
  version: string;
  enabled: boolean;
  trust: Trust;
  key?: string | null;
  publisher?: string | null;
  granted: string[];
  autoUpdate: boolean;
  pendingUpdate?: PendingUpdate | null;
  installedAt: number;
  source: string;
}
export interface InspectedPackage {
  token: string;
  manifest: ExtManifest;
  trust: Exclude<Trust, "builtin">;
  key: string | null;
  publisher: string | null;
  replaces: string | null;
  blocked: string | null;
}

type ListKey = "themes" | "buttonStyles" | "backgrounds" | "fonts" | "views" | "commands" | "settings";

export function list<K extends ListKey>(manifest: ExtManifest, key: K): NonNullable<ExtContributes[K]> {
  const value = manifest.contributes?.[key];
  return (Array.isArray(value) ? value : []) as NonNullable<ExtContributes[K]>;
}

export function validItemId(id: unknown): id is string {
  return typeof id === "string" && /^[A-Za-z0-9_-]{1,40}$/.test(id);
}

export function cleanLabel(value: unknown, max = 40): string {
  if (typeof value !== "string") return "";
  // Control and direction-changing characters do not belong in app labels.
  // eslint-disable-next-line no-control-regex
  return value.replace(/[\u0000-\u001f\u007f\u202a-\u202e]/g, "").trim().slice(0, max);
}
