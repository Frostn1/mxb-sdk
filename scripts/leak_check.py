"""Fail if this repository contains private implementation details or credentials.

Runs in CI on every push and pull request so leaks are caught before release.

    python scripts/leak_check.py
"""
import os
import re
import sys

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
SKIP_DIRS = {".git", "target", "node_modules", "dist"}
SELF = os.path.normpath(os.path.abspath(__file__))

DENY_PATHS = (
    "mxbsecure", "secure", "sidecar", "keylease", "antidebug", "track-protocol", "hooklib",
    "hookhost", "frostmod_hooks", "frostmod-hooks", ".pem", ".key", ".env", "id_rsa",
)
DENY_TEXT = [
    (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "a private key"),
    (r"\bgh[pousr]_[A-Za-z0-9]{30,}", "a GitHub token"),
    (r"\bgithub_pat_[A-Za-z0-9_]{30,}", "a GitHub token"),
    (r"\bnpm_[A-Za-z0-9]{30,}", "an npm token"),
    (r"\bcio[A-Za-z0-9]{30,}", "a crates.io token"),
    (r"\bsk-(ant|proj|live|test)[A-Za-z0-9_-]{16,}", "an API key"),
    (r"\bAKIA[0-9A-Z]{16}\b", "an AWS key"),
    (r"\bxox[abpr]-[A-Za-z0-9-]{10,}", "a Slack token"),
    (r"(?i)mxbsecure", "the private mxbsecure service"),
    (r"(?i)\bsecuresource\b|\bkeylease\b|\bantidebug\b", "a private module"),
    (r"\bKCOL\b|\b3KCL\b", "a private lock format"),
    (r"(?i)\bsidecar\b", "the private sidecar"),
    (r"(?i)frostmod[-_ ]?hooks|\bfmh_entry\b|FMH_ABI", "the private hook library"),
    (r"(?i)track[- ]protocol|track program", "the private track generator"),
    (r"(?i)frostmod-private|mxbapp-private|mxb-apps-private", "a private repo"),
    (r"(?i)\b[a-z0-9.-]+\.workers\.dev\b", "an internal worker URL"),
    (r"(?i)control[- ]plane", "the private control plane"),
    (r"(?i)apps[\\/]manager|src-tauri|packages[\\/]shared[\\/]|crates[\\/]core[\\/]src|gamesdk_replay|MXB_GAMESDK_REPLAY|sdk_split", "a private repository path or build name"),
    (r"(?i)frostmod", "an internal game-integration name"),
    (r"\b(?:10|192\.168|172\.(?:1[6-9]|2\d|3[01]))\.\d{1,3}\.\d{1,3}(?:\.\d{1,3})?\b", "a private address"),
]


def main():
    hits = []
    for base, dirs, files in os.walk(ROOT):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for name in files:
            path = os.path.join(base, name)
            if os.path.normpath(path) == SELF:
                continue
            rel = os.path.relpath(path, ROOT).replace("\\", "/")
            for d in DENY_PATHS:
                if d in rel.lower() and rel != ".gitignore":
                    hits.append(f"{rel}: denied path fragment {d!r}")
            try:
                with open(path, encoding="utf-8") as f:
                    text = f.read()
            except (UnicodeDecodeError, OSError):
                continue
            if rel == ".gitignore":
                continue
            for pattern, what in DENY_TEXT:
                for m in re.finditer(pattern, text):
                    line = text.count("\n", 0, m.start()) + 1
                    hits.append(f"{rel}:{line}: {what}")
    if hits:
        print("Private material found:")
        print("\n".join("  " + h for h in hits))
        return 1
    print("leak check: clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
