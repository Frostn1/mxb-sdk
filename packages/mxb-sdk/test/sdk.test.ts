import { expect, test } from "bun:test";
import { core, extensions, formatLap, game, mcp, plugins } from "../src/index";

test("exports package namespaces and common helpers", () => {
  expect(typeof core.findInstalls).toBe("function");
  expect(typeof game.GameSdk).toBe("function");
  expect(typeof extensions.validateManifest).toBe("function");
  expect(typeof plugins.listPlugins).toBe("function");
  expect(typeof mcp.McpClient).toBe("function");
  expect(formatLap(95123)).toBe("1:35.123");
});
