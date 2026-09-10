# skimmd

Rust CLI for reading markdown: generate a table of contents, extract line
ranges, and filter sections by substring. Library + binary crate.

## Toolchain
- Rust, pinned in `.tool-versions` (currently `1.97.1`). CI uses the same pin
  via `dtolnay/rust-toolchain` — keep the two in sync.
- `edition = 2024`; `rust-version = 1.85` is the floor users need to build from
  source / `cargo install` (we build with the pinned, newer toolchain).

## Quality gates (all must pass before work is "done")
CI (`.github/workflows/ci.yml`) fails on any of these; run them locally first:
- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`

Plus the sample drift check (captured outputs must stay in sync):
- `sh samples/regenerate.sh`  → must print `samples in sync`

## Conventions
- Keep this file up to date: if you change the toolchain pin, a quality gate,
  the layout, or the release steps, update it in the same change.
- Before finishing non-trivial work, run a ponytail review of the diff and
  apply the suggested simplifications.

## Layout
- `src/` — `lib.rs` + `main.rs`; modules `toc`, `ranges`, `lines`, `elide`, `format`
- `tests/` — `cli.rs` (CLI/integration), `invariants.rs`, `golden_toc.rs`, `fixtures/`
- `samples/` — captured inputs + outputs (`1`–`7`) and `regenerate.sh`
- `skill/SKILL.md` — the agent skill wrapping the CLI

## Releasing
Two channels, both driven by the `Cargo.toml` `version` + a matching `vX.Y.Z` tag:
1. All quality gates green (above) + `sh samples/regenerate.sh`.
2. `CHANGELOG.md`: move `## [Unreleased]` under a new `## [X.Y.Z] - <date>`;
   add a fresh empty `## [Unreleased]` on top.
3. Bump `version` in `Cargo.toml`; build so `Cargo.lock` refreshes.
4. Commit (e.g. `Release X.Y.Z`).
5. `git tag vX.Y.Z` (**must equal** the `Cargo.toml` version), then
   `git push origin main vX.Y.Z` → `release.yml` builds 6 binaries + GitHub Release.
6. `cargo publish` from the tagged commit (crates.io is **immutable** — a
   mistake means yank + a new version).

Then bump every doc that references a *concrete* version to the new release
version (README install examples: the `<version>` note, `vX.Y.Z` prose, and the
`curl`/`chmod`/`mv` commands). Don't touch:
- `CHANGELOG.md` historical entries (record of past releases)
- `Cargo.lock` dependency versions (Cargo-managed; only skimmd's own line moves)
- version-looking strings in content (e.g. DOIs/URLs in `samples/` articles)
