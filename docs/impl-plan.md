# skimmd — Implementation Plan (v1)

Source of truth: `docs/skimmd-spec.md` (complete v1 contract). This plan breaks the
contract into a few phases, each with a **measurable success gate** against a spec
section, so progress is verifiable by running.

## Environment (verified)

- Rust **1.98.0** (cargo, rustc, clippy, rustfmt all installed via rustup; default toolchain).
  `~/.cargo/bin` must be on `PATH`.
- Golden fixtures already present in `docs/golden-fixtures/`; **all SHA-256 hashes
  verified** against §11. `example.md` = 283 bytes, `N = 30`.
- License: **Apache-2.0** (owner choice).
- Crate: `skimmd`, edition **2024**, `rust-version = "1.85"`.

## Key decisions (and deliberate deviations from the §13 *suggestion*)

1. **lib + bin, not bin-only.** The spec's suggested layout is `src/main.rs` + modules.
   I split into a `skimmd` library (core: `lines`, `ranges`, `toc`, `format`) plus a thin
   `main.rs` binary. Rationale: the whole point of the tool is that an agent skill / MCP
   server wraps it later — a lib makes that a `use skimmd::{…}` import (wrappable from
   Rust) rather than only shelling out, and it's the shape a publish-ready crate takes.
   Public API is kept minimal (core functions, no gratuitous surface).
2. **`serde` (derive) added** alongside `serde_json`. Needed so `json` mode emits keys in
   the load-bearing order `line, level, end, chars, title` (serde preserves *struct field
   order*; a `serde_json::Map`/`Value` would alphabetize). Title escaping stays
   `serde_json` per spec. `serde`+`serde_json` is the canonical pair — not bloat.
3. **Fixtures live in `tests/fixtures/`** (copied once from `docs/golden-fixtures/`), so the
   test suite is self-contained. `docs/golden-fixtures/` stays as the human reference. A
   guard test re-checks `example.md`'s SHA-256 so the fixture can't drift silently.
4. **Lazy, non-collecting heading scan** (state machine over the offset iterator).
   `collect()`-ing every `OffsetItem` for a 10 MB file would blow the `<1 s`/memory budget.
5. **EPIPE**: rely on Rust std's `BrokenPipe` error (std sets `SIGPIPE`→ignore). Verified
   in Phase 4 with `| head`. No `libc`/`nix` dep (spec forbids new deps).
6. **README** documents only: what it does, usage, range grammar, and the two *required*
   documented limitations (§4.1 mid-doc `---`, §10 blank-first-line front matter). Per §14,
   out-of-scope features (TOML front matter, link counts, MCP, caching, …) are **not**
   mentioned in README/`--help`/code.

## Module layout

```
Cargo.toml, LICENSE (Apache-2.0), README.md, .gitignore, Cargo.lock (committed)
src/
  lib.rs      # pub mod lines, ranges, toc, format; thin public API
  main.rs     # clap CLI, mode dispatch, exit codes, stderr, stdout (BufWriter+EPIPE), load()
  lines.rs    # LineMap: starts, N, line_of, span, chars  (+ BOM strip / UTF-8 load)
  ranges.rs   # SPEC grammar parse, validation, normalization
  toc.rs      # pulldown-cmark setup, heading extraction, Row computation
  format.rs   # md / tsv / json emitters (serde structs for json)
tests/
  fixtures/   # example.md + expected_*.{md,tsv,json,txt} + SHA256SUMS
  cli.rs      # integration tests via assert_cmd (byte-exact + exit codes)
```

## Algorithm recap (verified by hand against §11 numbers)

- **Line model** (§3.2): `starts=[0]`, push `j+1` for each `\n` at `j` where `j+1<len`;
  `N = starts.len()` (0 if empty). `line_of(o) = starts.partition_point(|&s| s<=o)` (1-based).
  `span(a,b) = text[starts[a-1] .. end_of(b)]`, `end_of(b) = b<N ? starts[b] : len`.
- **Headings** (§5.2): state machine over `into_offset_iter()`. On `Start(Heading{level})`
  grab range → `first_line=line_of(start)`, `last_line=line_of(end-1)`; accumulate inline
  `Text/Code/Math`→append, `Soft/HardBreak`→space, inline tags & html/footnote→nothing, until
  `End(Heading)`. Normalize title: `split_whitespace().join(" ")`.
- **Rows** (§6): preamble row iff `N>0 && (k==0 || H[0].first_line>1)`, region `1..=P`.
  Per heading: `chars = chars(last_line+1, next.first_line-1 or N)`; `end = subtree_end` via
  a stack (pop while top.level ≥ cur.level → end=top before `cur.first_line-1`; leftovers=N).
- **Ranges** (§8): two-phase. Phase A syntax (per whole spec arg → `invalid range spec '…'`).
  Phase B validation in pool order → `range <raw>: …`. Then sort by eff-start, merge
  adjacent/overlapping. Output `span(a,b)` per merged range, no separators.

Hand-verified on the fixture: preamble `1-7/74`; ends `30,19,19,23,27,28,30`; body chars
`17,23,14,17,17,0,26`; invariant `74+95+114=283`. Matches all of §11.2/3/4.

## Risks to verify early

- **pulldown-cmark 0.13 API**: `into_offset_iter()`, `Options` flags
  (`ENABLE_YAML_STYLE_METADATA_BLOCKS`, `ENABLE_HEADING_ATTRIBUTES`,
  `ENABLE_STRIKETHROUGH`, `ENABLE_FOOTNOTES`), `Tag::Heading{level,..}`,
  `Event::InlineMath/DisplayMath/FootnoteReference`, `HeadingLevel → u8`.
  → Probe in Phase 0 with a tiny throwaway before committing to the shape.
- **`HeadingLevel as u8`**: use an explicit `match` (repr not guaranteed).

---

## Phases

### Phase 0 — Scaffold & dependency probe
Tasks: `cargo init`-equivalent by hand; `Cargo.toml` (name/version/edition/rust-version/
license Apache-2.0/description/categories; deps clap-4 derive, pulldown-cmark 0.13,
serde_json 1, serde derive; dev assert_cmd 2); `LICENSE` (Apache-2.0 full text);
`.gitignore`; `src/lib.rs`+empty modules; `src/main.rs` stub that prints version; copy golden
fixtures to `tests/fixtures/`; trivial smoke test.
**Gate:** `cargo build` clean; `cargo clippy --all-targets -- -D warnings` clean;
`cargo test` runs (≥1 green); `pulldown-cmark` 0.13 API probe compiles & prints expected.
Commit: `Scaffold crate: Cargo.toml, lib+bin, Apache-2.0, fixtures`.

### Phase 1 — Line model + input loading  (`lines.rs`)
Tasks: `LineMap` (starts/N/`line_of`/`span`/`chars`); `load()` BOM-strip + UTF-8 validate +
io-error classification (NotFound → "No such file or directory", IsADirectory → "is a
directory", else OS text).
**Gate:** §12.2 unit tests green (empty, `"\n"`, `"a"`, `"a\n"`, `"a\nb"`, `"a\n\n"`, CRLF,
lone `\r`) — line numbers match `grep -n`/`sed` semantics.
Commit: `Implement line model + input loading`.

### Phase 2 — Range parser  (`ranges.rs`)
Tasks: SPEC grammar (two-phase syntax+validation), normalization (sort+merge), literal-token
error messages.
**Gate:** §12.3 table-driven unit tests green — all good forms (`1-`,`1-1`,`007-010`,
`1-5,9-12`) normalize correctly; all error forms (`5`,`0-`,`0-0`,`10-5`,`1-10,x-y`,`1-` on
empty) yield the exact §9 messages; merge of `5-10,3-6` == `3-10`.
Commit: `Implement range grammar, validation, normalization`.

### Phase 3 — TOC engine  (`toc.rs` + `format.rs`)
Tasks: parser setup (exact `Options`), heading state machine, Row computation (preamble +
stack-based subtree_end), md/tsv/json emitters.
**Gate:** run against `example.md` — `--format md|tsv|json` outputs **byte-identical** to
`tests/fixtures/expected_toc.{md,tsv,json}` (asserted). Accounting invariant (§6.2) holds.
Commit: `Implement TOC computation and md/tsv/json output`.

### Phase 4 — CLI wiring, errors, range-mode output  (`main.rs`)
Tasks: clap derive (file + variadic ranges, `--format`), mode dispatch, `die()` stderr
helper, exit codes (1 self / 2 clap), BufWriter stdout + EPIPE→0, range-mode write path.
**Gate:** §11.5 range outputs byte-exact (`1-4`,`12-19`,`29-`,`8-8 16-`,`1-`, merge case);
all §9 error templates exact; §12.10 exit codes (missing file/dir/bad-utf8/bad-format/
missing-FILE/`0-`); `| head` → exit 0 silent.
Commit: `Wire CLI, error handling, and range-mode output`.

### Phase 5 — Full test suite + fuzz  (`tests/cli.rs` + more unit)
Tasks: golden integration tests (byte compare); §12.4 setext-trap; §12.5 blockquote/list/code;
§12.6 heading-attribute; §12.7 setext multi-line (+leading-blank variant); §12.8 invariant
property (a)–(d); §12.9 no-panic fuzz (loop over random bytes/UTF-8); §12.10 exit-code tests.
**Gate:** `cargo test` fully green (unit + integration + fuzz).
Commit: `Add full test suite (golden, edge cases, invariants, fuzz)`.

### Phase 6 — Publish polish + review
Tasks: full README (usage, range grammar, 2 required limitations, no out-of-scope mentions);
doc comments; `cargo fmt`; clippy `-D warnings` clean across all targets; generate a ~10 MB
file and confirm `<1 s`; **ponytail-review** against the spec; final progress-log entry.
**Gate:** `cargo fmt --check` clean; clippy clean; `cargo test` green; 10 MB < 1 s; no spec
violations / no out-of-scope leakage found in review.
Commit: `Polish: README, docs, fmt/clippy clean, perf verified` (+ any review fixes).

## Progress tracking
Running log: `docs/impl-progress.md` (append after each phase; re-read it + this plan if
lost). Compaction after each phase to keep context low.
