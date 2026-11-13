"""Fail when public client sources contain a game write path."""

from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parent.parent
SOURCE_ROOTS = (
    "crates/mxb-game/src",
    "packages/mxb-game/src",
    "packages/mxb-game/samples",
    "packages/mxb-game/bin",
    "crates/mxb-mcp/src",
    "packages/mxb-mcp/src",
    "packages/mxb-mcp/bin",
    "extensions/samples",
)
MCP_POST_ROOTS = (
    "crates/mxb-mcp/src/",
    "packages/mxb-mcp/src/",
    "packages/mxb-mcp/bin/",
    "packages/mxb-game/bin/mxb-game-mcp.mjs",
)
DENIED = (
    (re.compile(r"/v1/actions"), "/v1/actions"),
    (re.compile(r"method\s*:\s*[\"']POST[\"']"), 'method: "POST"'),
    (re.compile(r"\.post\s*\("), ".post("),
    (re.compile(r"reload_mods"), "reload_mods"),
    (re.compile(r"refresh_content"), "refresh_content"),
    (re.compile(r"WriteProcessMemory|SendInput|keybd_event|vJoy|OpenProcess"), "game input or process API"),
)


def main() -> int:
    hits: list[str] = []
    files = {
        path
        for source in SOURCE_ROOTS
        for path in (ROOT / source).rglob("*")
        if path.is_file()
    }
    for path in sorted(files):
        relative = path.relative_to(ROOT).as_posix()
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (UnicodeDecodeError, OSError):
            continue
        mcp_post = any(relative == root or relative.startswith(root) for root in MCP_POST_ROOTS)
        for number, line in enumerate(lines, 1):
            for index, (pattern, label) in enumerate(DENIED):
                if mcp_post and index in (1, 2):
                    continue
                if pattern.search(line):
                    hits.append(f"{relative}:{number}: {label}")
    if hits:
        print("Game write boundary violations:")
        print("\n".join(f"  {hit}" for hit in hits))
        return 1
    print("boundary check: clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
