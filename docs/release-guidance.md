# Releasing skimmd

This is the owner's manual for cutting a release. It covers the two distribution
channels, the one-time setup, the per-release steps, and the gotchas. If you just
want the commands, jump to **Quick start**.

## The two channels

We ship through two channels, both driven by the `version` in `Cargo.toml` and a
matching `vX.Y.Z` git tag:

1. **crates.io** — the canonical Rust package registry. Users run
   `cargo install skimmd`, which compiles it from source with *their* Rust
   toolchain. Publishing is the `cargo publish` command.
2. **GitHub Releases** — prebuilt binaries (Linux / macOS / Windows) for people who
   don't have Rust. The workflow at `.github/workflows/release.yml` builds them
   automatically the moment you push a `vX.Y.Z` tag, and attaches them to a GitHub
   Release.

## One-time setup (do this once, before the first release)

### 1. Push the repo to GitHub

```sh
git remote add origin git@github.com:YOURNAME/skimmd.git   # or the https URL
git push -u origin main
```

Once the repo exists, the two Actions workflows (`ci.yml`, `release.yml`) are live.

### 2. Replace the `yourname` placeholder

I put a `yourname` placeholder in two spots so I didn't have to guess your handle.
Replace it with your real GitHub username (and the repo name if it isn't `skimmd`):

- `Cargo.toml` → the `repository` and `homepage` fields.
- `README.md` → the "GitHub Releases" link in the Install section.

Commit the change.

### 3. Get a crates.io account + a publish token

1. Make an account at <https://crates.io> (sign in with your GitHub — that's also
   what claims the `skimmd` name for you).
2. Create an API token: <https://crates.io/settings/tokens> → **Create token** →
   scope it to `publish_new_version` for the crate `skimmd`. Copy the token.
3. Store it locally so `cargo publish` can use it:

   ```sh
   # Option A — one-time login (cargo remembers it):
   cargo login YOUR_TOKEN

   # Option B — persistent config (survives, works for scripts):
   mkdir -p ~/.cargo
   printf '[registry.crates-io]\ntoken = "YOUR_TOKEN"\n' >> ~/.cargo/credentials.toml
   ```

   Either is fine; pick A for simplicity. For a one-off (or in CI) you can also pass
   `cargo publish --token YOUR_TOKEN` directly.

## Quick start: cutting a release

Assuming the one-time setup is done and `Cargo.toml` says `version = "0.1.0"`:

```sh
# 1. Make sure it's green (CI checks this too, but confirm locally first).
cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check

# 2. Set the version you're releasing in Cargo.toml.
#    (First release is already 0.1.0, so you can skip this the first time.)
#    e.g. later:  version = "0.2.0"

# 3. Let any build refresh Cargo.lock, then commit everything.
cargo build --release
git add -A && git commit -m "Release 0.1.0"

# 4. Tag (the tag MUST equal the version) and push.
git tag v0.1.0
git push origin main v0.1.0
#   -> GitHub Actions builds the 3 binaries and opens the GitHub Release.

# 5. Publish to crates.io, from the tagged commit.
cargo publish
#   -> skimmd 0.1.0 is live; `cargo install skimmd` now works for users.
```

That's the whole thing: **the tag triggers the binaries, `cargo publish` does
crates.io.** Do them in that order (commit → tag → push → publish).

## What to tell users

Ready to paste into an announcement or README:

> **Install `skimmd`**
> - **If you have Rust (≥ 1.85):** `cargo install skimmd`
>   (faster, if you have `cargo-binstall`: `cargo binstall -i skimmd`)
> - **If you don't:** download the latest
>   `skimmd-<version>-linux` / `-macos` / `-windows.exe` from the [GitHub
>   Releases](https://github.com/YOURNAME/skimmd/releases) page, then put it on
>   your `PATH` (`chmod +x` it on Linux/macOS first).
> - Run `skimmd --help` for usage.

## Versioning, briefly

- Versions are `MAJOR.MINOR.PATCH` (`0.1.0`).
- While you're at `0.x`, breaking changes are expected: bump **minor** for new
  features, **patch** for fixes. Once the API is stable and you mean it, go to
  `1.0.0`.
- **crates.io versions are immutable** — you can never publish the same version
  twice. Made a mistake? Bump to the next number (e.g. `0.1.1`), publish that, and
  optionally `cargo yank --version 0.1.0` the bad one (yank hides it from fresh
  installs without deleting it).
- **Tag == version.** `v0.1.0` ⇔ `version = "0.1.0"`. The release workflow names
  the binaries from the tag, and `cargo publish` uses the version in `Cargo.toml`,
  so keep them identical.

## Gotchas (the ones that actually bite)

- **`cargo publish` publishes the commit you're standing on.** Run it from the
  tagged commit so the published version matches the tag.
- **Bump → build → commit.** `Cargo.lock` pins exact dependency versions. After a
  version bump, run a `cargo build`/`cargo test` so the lockfile is current, and
  commit it (it's committed here because this is a binary crate).
- **The first `cargo publish` is interactive.** It prints the manifest and the file
  list and asks `is this ok?` — answer `y`. Preview without uploading with
  `cargo publish --dry-run`.
- **Toolchain floor.** `edition = "2024"` / `rust-version = "1.85"` means building
  from source needs Rust ≥ 1.85. `cargo install` uses the user's installed toolchain,
  so state the floor in the install docs (already in the README).
- **Re-cutting a release.** Git tags are (effectively) immutable. To redo one:
  `git tag -f v0.1.0 && git push -f origin v0.1.0` — but prefer just cutting the
  *next* version for anything non-trivial.
- **Badges.** The README's crates.io/docs.rs badges 404 until the first publish;
  they light up automatically after that.

## Optional automation (add later, not needed for release 1)

- **Publish crates.io from CI.** Add a `CRATESIO_TOKEN` repository secret
  (Settings → Secrets and variables → Actions) and a step in `release.yml`:
  `cargo publish --token "$CRATESIO_TOKEN"`. Then a single `git push` of the tag
  does both channels.
- **`cargo-dist`.** The Rust team's distribution tool: `cargo install cargo-dist &&
  cargo dist init`. It manages the whole multi-platform release and can add
  Homebrew taps and more. Heavier — nice once you're past release 1.
- **`cargo-release`.** Automates the version-bump + commit + tag dance:
  `cargo install cargo-release`.

## What I set up for you (already committed)

| File | What it does |
|---|---|
| `Cargo.toml` | `repository` / `homepage` / `documentation` (fill `yourname`) |
| `.github/workflows/ci.yml` | `fmt` + `clippy` + `test` on every push/PR |
| `.github/workflows/release.yml` | on a `v*` tag → build 3 binaries → GitHub Release |
| `README.md` | badges + install instructions (crates.io / Releases / source) |

## What you can't do locally (needs a live remote)

Pushing to GitHub, running the Actions workflows, and `cargo publish` all need
network access plus a GitHub remote and a crates.io token. Everything short of
those is done — once the repo is pushed and your token is set, the Quick-start
commands are all that's left.
