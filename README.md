# skimmd - read markdown with speed and precision

[![crates.io](https://img.shields.io/crates/v/skimmd.svg)](https://crates.io/crates/skimmd)
[![docs.rs](https://docs.rs/skimmd/badge.svg)](https://docs.rs/skimmd)

`skimmd` prints a markdown file's table of contents, optionally filtered to the sections you care about,
and extracts exactly the lines you want — built for agents, useful for anyone.

## Quick demo

Say you're researching pollution and its effects on rain. You've
downloaded [Wikipedia's article on Rain](https://en.wikipedia.org/wiki/Rain)
and converted it to markdown — 260KB. Now you need just the parts about
your topics. Reading the whole thing wastes context and tokens, and
`grep`/`rg` are too blunt to pull out sections at the right granularity.

### First, find candidate sections

List the article's sections, filtered to the terms you care about.
Matches are case-insensitive substrings, and `|` ORs multiple terms, so
cast a wide net first — any section whose title or content contains one
of your terms:

```
$ skimmd samples/1.full-article.md -f "acid|pollut"
| line | level | end | chars | title | matches |
|---|---|---|---|---|---|
| 128 | 2 | 199 | 3152 | Contents | 4 |
| 445 | 3 | 472 | 5083 | Human influence | 3 |
| 518 | 3 | 534 | 1979 | Acidity | 21 |
| 589 | 3 | 630 | 5255 | Pollution and composition | 17 |
| 1121 | 2 | 2383 | 126347 | References | 9 |
```

### Then, extract just the sections you want

The _Contents_ and _References_ sections are irrelevant to your question,
so you extract only the three you need — using the `line`/`end` columns
as ranges, in a single command:

```
$ skimmd samples/1.full-article.md 445-472,518-534,589-630
### Human influence

The fine particulate matter produced by car exhaust and other human sources of
pollution forms cloud condensation nuclei …

### Acidity

…

### Pollution and composition

…
```

That's ~12k characters out of 260KB — about 1/20th the size.

The full walkthrough is in [samples/](samples/README.md); the full CLI
is in [Usage](#usage).

## Install

**Prebuilt binary** (no Rust required). Download the file for your platform and
CPU from the latest [GitHub Release](https://github.com/cwilper/skimmd/releases);
`<version>` below is the release version (e.g. `0.2.1`):

| Platform | File |
|---|---|
| Linux x86_64 | `skimmd-<version>-x86_64-unknown-linux-musl` |
| Linux ARM64 (aarch64) | `skimmd-<version>-aarch64-unknown-linux-musl` |
| macOS Intel (x86_64) | `skimmd-<version>-x86_64-apple-darwin` |
| macOS Apple Silicon (aarch64) | `skimmd-<version>-aarch64-apple-darwin` |
| Windows x86_64 | `skimmd-<version>-x86_64-pc-windows-msvc.exe` |
| Windows ARM64 (aarch64) | `skimmd-<version>-aarch64-pc-windows-msvc.exe` |

Linux builds are statically linked (musl), so they run with no system C runtime.
Download and install one — e.g. Linux x86_64, v0.2.1:

```sh
curl -LO https://github.com/cwilper/skimmd/releases/download/v0.2.1/skimmd-0.2.1-x86_64-unknown-linux-musl
chmod +x skimmd-0.2.1-x86_64-unknown-linux-musl
sudo mv skimmd-0.2.1-x86_64-unknown-linux-musl /usr/local/bin/skimmd
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
  -f, --filter <SUBSTRING> Filter TOC rows: `|`-separated substrings, a row matches if any occurs in its heading or body (case- and whitespace-insensitive; substring match — `foo` also matches `food`; TOC mode only)
  -h, --help             Help
  -V, --version          Version
```

### TOC mode

```
$ skimmd samples/1.full-article.md
| line | level | end | chars | title |
|---|---|---|---|---|
| 1 | 0 | 40 | 1311 | preamble |
| 41 | 1 | 2913 | 6797 | Rain |
| 128 | 2 | 199 | 3152 | Contents |
| 200 | 2 | 332 | 1 | Formation |
| 203 | 3 | 239 | 4489 | Water-saturated air |
| … |
| 2384 | 2 | 2913 | 1384 | External links |
| 2413 | 4 | 2913 | 22084 | Languages |
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

### Filter TOC rows

`--filter` (`-f`) keeps only the sections whose **heading or body text** contains at
least one candidate substring. You can supply several, separated by `|` — a row is kept
if **any** of them matches (e.g. `-f 'install|build'`). `|` is a literal separator,
not a regex; leading, trailing, and repeated pipes are ignored. Each candidate is
matched case-insensitively, whitespace-insensitively, and as a **substring** (not a
whole word — so `foo` also matches `food`). Every run of whitespace (spaces, tabs,
newlines) collapses to a single space on both sides, so a candidate can match across a
line break. It is how you jump straight to the sections you care about instead of
scanning the whole TOC. The output is the filtered TOC plus a `matches` column —
the total occurrences of your candidate substrings in that section (heading + body,
summed over all candidates), a ranking signal for which hits are meaty sections and
which only brush the topic:

```
$ skimmd samples/1.full-article.md -f "acid|pollution"
| line | level | end | chars | title | matches |
|---|---|---|---|---|---|
| 128 | 2 | 199 | 3152 | Contents | 4 |
| 445 | 3 | 472 | 5083 | Human influence | 3 |
| 518 | 3 | 534 | 1979 | Acidity | 21 |
| 589 | 3 | 630 | 5255 | Pollution and composition | 14 |
| 1121 | 2 | 2383 | 126347 | References | 7 |
```

Because a section's *text* is searched (not just the heading), a substring that
appears only in the body still matches. Subsections are matched on their own
text, so a hit points at the tightest range to fetch. No match prints an empty
TOC (header only) and exits `0` — "nothing matched" is not an error.

### Range mode

```
$ skimmd samples/1.full-article.md 518-520
### Acidity

[![](https://thumb.wikimedia.org/…)](https://en.wikipedia.org/wiki/File:Origins_of_acid_rain.svg.png)

$ skimmd samples/1.full-article.md 589-630
### Pollution and composition

Aside from contamination of rainwater by [sulfuric](…) and [nitric
oxides](…), which produces acid rain, various pollutants from industry
and household wastes can end up in rainwater …
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
| 2 | Usage error (bad flag). |

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
