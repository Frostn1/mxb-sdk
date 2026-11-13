export * as core from "../../mxb-core/src/index";
export * as game from "../../mxb-game/src/index";
export * as extensions from "../../mxb-extensions/src/index";
export * as plugins from "../../mxb-plugins/src/index";
export * as mcp from "../../mxb-mcp/src/index";

export { findInstalls, identify, install, listMods, plan, uninstall } from "../../mxb-core/src/index";
export { GameSdk, GameSdkError, formatLap } from "../../mxb-game/src/index";
export { assertManifest, keygen, pack, validateManifest } from "../../mxb-extensions/src/index";
export { listPlugins, readPluginManifest } from "../../mxb-plugins/src/index";
export { McpClient, McpError } from "../../mxb-mcp/src/index";
