"""Fail when a relative Markdown link does not resolve inside the repository."""

import os
import re
import sys
from urllib.parse import unquote


ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
SKIP_DIRS = {".git", "target", "node_modules", "dist"}
INLINE_LINK = re.compile(r"!?\[[^\]]*\]\(\s*(?:<([^>]+)>|([^\s)]+))")
REFERENCE_LINK = re.compile(r"^\s*\[[^\]]+\]:\s*(?:<([^>]+)>|(\S+))", re.MULTILINE)
SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")


def markdown_files():
    for base, dirs, files in os.walk(ROOT):
        dirs[:] = [name for name in dirs if name not in SKIP_DIRS]
        for name in files:
            if name.lower().endswith(".md"):
                yield os.path.join(base, name)


def link_targets(text):
    for pattern in (INLINE_LINK, REFERENCE_LINK):
        for match in pattern.finditer(text):
            yield match.group(1) or match.group(2), text.count("\n", 0, match.start()) + 1


def main():
    broken = []
    for source in markdown_files():
        with open(source, encoding="utf-8") as handle:
            text = handle.read()
        for raw_target, line in link_targets(text):
            target = raw_target.strip()
            if (
                not target
                or target.startswith("#")
                or target.startswith("//")
                or target.startswith("/")
                or SCHEME.match(target)
            ):
                continue
            file_part = unquote(target.split("#", 1)[0].split("?", 1)[0])
            if not file_part:
                continue
            resolved = os.path.normpath(os.path.join(os.path.dirname(source), file_part))
            if not os.path.isfile(resolved):
                rel_source = os.path.relpath(source, ROOT).replace("\\", "/")
                broken.append(f"{rel_source}:{line}: {target}")
    if broken:
        print("Broken relative Markdown links:")
        print("\n".join("  " + item for item in broken))
        return 1
    print("link check: clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
