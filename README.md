# skimmd - Skim Markdown at agent speed

[![crates.io](https://img.shields.io/crates/v/skimmd.svg)](https://crates.io/crates/skimmd)
[![docs.rs](https://docs.rs/skimmd/badge.svg)](https://docs.rs/skimmd)

`skimmd` is a small CLI for **agent-driven navigation of a single Markdown file**.
It has two modes:

- **TOC mode** — `skimmd FILE` prints the file's structure as a table of
  contents with line numbers and sizes, so you (or an agent) can see the whole
  document at a glance and jump straight to the relevant line range.
- **Range mode** — `skimmd FILE RANGE...` prints the requested line ranges
  **verbatim**, byte for byte.

It is deliberately narrow: one file, plain stdout, stable and predictable
output. No rendering, no HTML, no opinionated defaults.

## Install

**Prebuilt binary** (no Rust required). Download the file for your platform and
CPU from the latest [GitHub Release](https://github.com/cwilper/skimmd/releases);
`<version>` below is the release version (e.g. `0.1.0`):

| Platform | File |
|---|---|
| Linux x86_64 | `skimmd-<version>-x86_64-unknown-linux-musl` |
| Linux ARM64 (aarch64) | `skimmd-<version>-aarch64-unknown-linux-musl` |
| macOS Intel (x86_64) | `skimmd-<version>-x86_64-apple-darwin` |
| macOS Apple Silicon (aarch64) | `skimmd-<version>-aarch64-apple-darwin` |
| Windows x86_64 | `skimmd-<version>-x86_64-pc-windows-msvc.exe` |
| Windows ARM64 (aarch64) | `skimmd-<version>-aarch64-pc-windows-msvc.exe` |

Linux builds are statically linked (musl), so they run with no system C runtime.
Download and install one — e.g. Linux x86_64, v0.1.0:

```sh
curl -LO https://github.com/cwilper/skimmd/releases/download/v0.1.0/skimmd-0.1.0-x86_64-unknown-linux-musl
chmod +x skimmd-0.1.0-x86_64-unknown-linux-musl
sudo mv skimmd-0.1.0-x86_64-unknown-linux-musl /usr/local/bin/skimmd
```

**`cargo install`** (if you have Rust ≥ 1.85):

```sh
cargo install skimmd
# faster, if you have cargo-binstall:  cargo binstall -i skimmd
```

**From source** (clone the repo, then `cargo build --release` — the binary lands
in `target/release/skimmd`).

## Usage

```
skimmd [OPTIONS] [FILE] [RANGE]...

Arguments:
  [FILE]     Path to a Markdown file, or - to read from stdin. Omit it (or use -) to read from standard input.
  [RANGE]... Zero or more line ranges; any range switches to range mode

Options:
  -f, --format <FORMAT>  TOC output format: md, tsv, or json (default: md)
  -h, --help             Help
  -V, --version          Version
```

### TOC mode

```
$ skimmd docs/example.md
| line | level | end | chars | title |
|---|---|---|---|---|
| 1 | 0 | 7 | 74 | preamble |
| 8 | 1 | 30 | 17 | Project |
| 12 | 2 | 19 | 23 | Install |
| 16 | 3 | 19 | 14 | macOS |
| 20 | 2 | 23 | 17 | Usage | Notes |
| 24 | 2 | 27 | 17 | Setext Heading |
| 28 | 2 | 28 | 0 | Empty |
| 29 | 2 | 30 | 26 | Last |
```

Each row is a section:

- `line` — the line the section's heading starts on (the leading preamble —
  anything before the first heading — is a level-0 row titled `preamble`).
- `level` — heading depth (1–6); the preamble is level 0.
- `end` — the last line of the section's subtree (its heading and everything up
  to the next heading of equal or shallower level).
- `chars` — the character count of the section's body (after its own heading
  line(s), before the next heading).
- `title` — the heading text with Markdown emphasis/code/links stripped and
  internal whitespace collapsed to single spaces.

`--format tsv` gives the same rows as tab-separated fields (no header);
`--format json` gives a JSON array of `{line, level, end, chars, title}` objects.

### Range mode

```
$ skimmd FILE 1-4
---
title: Example
tags: [a, b]
---

$ skimmd FILE 12-14
## Install

Run `cargo install`.

$ skimmd FILE 28-29
## Empty
## Last
```

**Range grammar:** `N-M` (both inclusive) or `N-` (through the last line).
Ranges are comma- and/or space-separated, in any mix: `1-5,9-12`, `1-5 9-12`,
or `1-5,9-12 20-`. Overlaps merge, ranges are emitted in ascending order, and
`skimmd FILE 1-` reproduces the file exactly (byte-identical after a UTF-8 BOM
is dropped). Piped input works too: use `-` as the file (`cat FILE | skimmd -
1-4`).

## Behavior worth knowing

- **A UTF-8 BOM** is dropped from the first line but never counted or shown.
- **YAML front matter** at the top of the file is treated as body (it is not a
  heading); the leading preamble row still starts at line 1.
- **Setext headings** (`Title\n===`) and **ATX headings** both work. Headings
  inside code blocks, blockquotes, or lists are *not* TOC entries.
- **Line numbers are 1-based** over the raw file lines.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. |
| 1 | The request could not be satisfied: file not found / is a directory / not valid UTF-8, an invalid range, or a non-`EPIPE` stdout write error. |
| 2 | Usage error (bad flag, unknown `--format`). |

Errors are printed to stderr as `skimmd: <message>`; on any error, stdout is
empty. A broken pipe (`EPIPE`) is a clean exit 0 with no message.

## Development

```sh
cargo test          # unit + integration + golden + invariant + fuzz
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## License

Apache-2.0. See [LICENSE](LICENSE).
