# skimmd — Implementation Progress Log

Append-only running log. Re-read this + `docs/impl-plan.md` if I get confused.

## Setup / recon (before Phase 0)

- Read full spec `docs/skimmd-spec.md`. It is a complete, self-consistent v1 contract.
- Environment: `rustc`/`cargo` were NOT on PATH. The earlier `mise x rust` one-shot had run
  `rustup-init` but the toolchain download was interrupted. **Fixed:** completed
  `rustup toolchain install 1.98.0` (default toolchain). `~/.cargo/bin` on PATH gives
  cargo/rustc/clippy/rustfmt. (mise did not persist a rust tool; rustup did.)
- **Golden fixtures already provided** in `docs/golden-fixtures/`; verified every SHA-256
  against §11 (all match). `example.md` = 283 B, `N=30`.
- **Owner decision (my one question):** license = **Apache-2.0** (single).
- Plan committed: `docs/impl-plan.md`.
- Hand-verified the core algorithm against §11 numbers before coding (preamble `1-7/74`;
  ends `30,19,19,23,27,28,30`; body chars `17,23,14,17,17,0,26`; invariant `74+95+114=283`).
  All match → confidence the plan's algorithm is correct.

### Decisions / findings
- Use **lib + bin** (deviation from §13 suggestion) so the crate is wrappable in Rust later.
- Add **serde (derive)** with serde_json so `json` keys stay in load-bearing order.
- Fixtures copied to `tests/fixtures/` (self-contained tests); `docs/golden-fixtures/` stays as reference.
- Keep timeouts short on install/poll commands (owner feedback).

## Phase 0 — Scaffold & dependency probe
- [x] **DONE.** `cargo build` clean; `cargo clippy --all-targets -- -D warnings` clean;
  `cargo test` green (version smoke); `skimmd --version` → `skimmd 0.1.0`.
- Created: `Cargo.toml` (lib+bin, Apache-2.0, edition 2024, rust-version 1.85, release
  profile), `LICENSE` (canonical Apache-2.0 text, © Chris Wilper 2026), `.gitignore` (`/target`),
  `src/{lib,main,lines,ranges,toc,format}.rs`, `tests/cli.rs` (smoke), fixtures copied to
  `tests/fixtures/`. Deps resolve: clap 4.6.6, pulldown-cmark **0.13.4**, serde 1.0.229,
  serde_json 1.0.151; dev assert_cmd 2.2.2 + predicates 3.1.4.

### pulldown-cmark 0.13 API findings (probe, then deleted)
- `Parser::new_ext(text, opts).into_offset_iter()` yields **`(Event<'a>, Range<usize>)`** —
  Event **first**, then a **concrete** `Range<usize>` (NOT `Option`, NOT `ByteRange`).
- `Tag::Heading { level, id, classes, attrs }` — no `info` field (it's `attrs`). Use `..`.
  `level: HeadingLevel` → map to `u8` via `match` (repr not guaranteed).
- Attributes (ENABLE_HEADING_ATTRIBUTES): valid `{#id .cls}` is stripped from the title
  (`# Real H1 {#id .cls}` → `Text("Real H1")`); invalid brace content (`{this}`) is KEPT.
  I just append `Text` events → correct in all cases, no special-casing.
- **Footnote ref in a heading**: `FootnoteReference("1")` event ONLY when a `[^1]:` definition
  exists elsewhere (→ dropped by design). Undefined `[^1]` → literal `Text("[","^1","]")`
  (kept in title). Both are exactly what "append Text, drop FootnoteReference" gives → spec-consistent.
- Metadata block: `Tag::MetadataBlock(YamlStyle)`, inner content is `Text` (no headings);
  consumed → the Setext trap is avoided. Front-matter lines belong to the preamble region.
- Blockquote/list headings: heading range starts at the `#` (after `> `/`- `), so
  `line_of(range.start)` = the container's line; `level` = `#` count.
- Setext: heading range spans the content line(s) **through the underline's EOL**.
- ATX heading range covers the `#…` line through its EOL (range.end just past the `\n`).
- `Event::InlineMath`/`DisplayMath` variants exist (compile).

## Phase 1 — Line model + input loading  (`lines.rs`)
- [x] **DONE.** `LineMap` (starts/N/`line_of` via `partition_point`/`span`/`chars`),
  `load_text()` (BOM strip + UTF-8 validate + `LoadErr`), `io_reason()` (exact phrases).
  15 unit tests green (covers §12.2 incl. CRLF, lone `\r`, multibyte, span guards).
  Confirmed `std::fs::read("/")` → `ErrorKind::IsADirectory` on Linux. clippy+fmt clean.
  Note: `span(a,b)` guards `a<1 || a>b || a>n` → `""` (avoids the out-of-bounds panic a
  `last_line==n` body would trigger via `body_a=n+1`).

## Phase 2 — Range parser  (`ranges.rs`)
- [x] **DONE.** Two-phase `parse(specs, n) -> Result<Vec<Range>, RangeErr>`: (A) syntax over
  every spec → `invalid range spec '{spec}'`; (B) validation in pool order → `range {raw}: …`
  (4 checks in order); (C) normalize (resolve `N-`→n, sort by start, merge adjacent/overlap).
  `Range{start,end}` (1-based inclusive, effective). `RangeErr::message()` returns the exact
  body after `skimmd: `. Leading zeros preserved in messages via raw tokens.
- Judgment: mixed syntax+validation across specs → parse-all-first (matches §8.1 "parse every
  spec fully before output"); not covered by any golden test.
- `parse_usize` saturates to `usize::MAX` on overflow so 25-digit line numbers yield the
  `exceeds file length` message, never a panic.
- Gate: 7 range tests green (all §12.3 forms, exact §9 messages, merge/adjacent/overlap,
  pool-order, empty-file, huge-number). clippy+fmt clean.

## Phase 3 — TOC engine  (`toc.rs` + `format.rs`)
- [ ] not started
