# Contributing

Thanks for wanting to help.

## How changes get in

This repository is a published copy of the SDK's packages from the private monorepo MXB App is
built in. Every sync replaces the tree, so a commit pushed here directly would be overwritten.

- **Issues** are welcome here: bugs, missing features, docs that are wrong or unclear.
- **Pull requests** are welcome too. They are reviewed here, then applied upstream by a
  maintainer and arrive back in the next sync with you credited as the author.

## Before you open a pull request

```bash
cargo test --workspace
cargo build -p mxb-sdk --no-default-features --features core
npm install && npm test
python scripts/leak_check.py
python scripts/boundary_check.py
```

- Keep the public API small: one obvious way to do each thing.
- Every public item gets a doc comment.
- Changes to the game data shapes start in `schema/game-sdk.schema.json`; run
  `node scripts/gamesdk/codegen.mjs` to regenerate the Rust and TypeScript types.
- Follow the versioning rules in [docs/sdk/versions.md](docs/sdk/versions.md).

## What will not be accepted

- Anything that sends input, commands or settings into the game, or that gives an advantage online.
- Code for protected or encrypted content formats.
- Anything that deletes user files instead of using the Recycle Bin.

## Security

Report a security problem privately through GitHub's "Report a vulnerability" on this
repository, not in a public issue.
