/** Permissions accepted by extension manifests and enforced by the app bridge. */
export const PERMISSIONS = [
  "mods.read",
  "servers.read",
  "friends.presence",
  "game.state",
  "game.read",
  "presets.read",
  "coach.read",
  "studio.launch",
  "network",
  "openExternal",
  "mcp.tools",
] as const;

export type Permission = (typeof PERMISSIONS)[number];

export function isPermission(permission: unknown): permission is Permission {
  return (PERMISSIONS as readonly unknown[]).includes(permission);
}

/** Permissions that read other people's data. Shown unticked by default. */
export const SENSITIVE: readonly Permission[] = ["friends.presence"];

/** Which bridge methods each permission opens. Everything not listed here is free. */
export const METHOD_PERMISSION: Record<string, Permission> = {
  "mods.list": "mods.read",
  "servers.list": "servers.read",
  "friends.list": "friends.presence",
  "game.state": "game.state",
  "game.read": "game.read",
  "game.stream": "game.read",
  "presets.list": "presets.read",
  "coach.sessions": "coach.read",
  "studio.open": "studio.launch",
  "ui.openExternal": "openExternal",
  "mcp.result": "mcp.tools",
};
