# skimmd — Implementation Retrospective

*Written after the fact, while it's fresh. Goal: what was true about the process,
where the spec and reality diverged, and what to carry into the next similar job.*

## Outcome

`skimmd` shipped as a publish-ready crate: a Rust lib + binary, 6 implementation
phases, **74 tests green** (45 unit, 21 binary, 4 golden-TOC, 4 invariant/fuzz),
clippy + fmt clean, `cargo package` verifying. An 8 MB file processes in ~11 ms
against a <1 s-for-10 MB target. Nine commits on top of the pre-existing spec
history, clean tree. The ponytail-review pass at the end found exactly one real
over-engineering item (a test-only helper that had become public API) and I fixed
it. Along the way I found one genuine spec bug (§5.2's worked example is
imprecise) and worked around a library constraint (pulldown-cmark's `bitflags`
`|` is not `const`).

The short version: **it went unusually well, and the reason was mostly that the
spec was unusually good.** A lot of the "how to do this kind of work well" lesson
is really about what to expect from (and ask for) a spec.

## The spec was the biggest variable

This spec read like a real contract, not a wishlist. It pinned exact stderr
strings, exact exit codes, the full error-ordering semantics of the range parser,
byte-for-byte golden outputs *with SHA-256 hashes*, and a test plan (§12) that
enumerated the properties to verify. That is a large share of implementation risk
removed up front: I almost never had to guess what "correct" meant.

What the spec did *not* fully cover, and where I had to exercise judgment:

1. **§5.2's worked example is wrong.** It claims `## a *b*c* d` yields title `abc d`.
   The real pulldown-cmark 0.13.4 output is `a bc* d` (the unclosed `*` stays
   literal and inter-element spaces are `Text`). The spec's *algorithm* (append
   `Text`/`Code`/`Math`, collapse breaks, drop inline tags) is unambiguous and is
   what I implemented; the one-line example just doesn't match its own algorithm.
   The golden fixtures contain no such heading, so behavior was verifiable without
   the example. **This is the kind of thing that would have turned into a
   "why does my output differ from the spec" mystery later if I hadn't recorded it
   the moment I saw it** — I put it in the progress log immediately.

2. **A dependency constraint forced a small non-idiomatic shape.** `parser_opts()`
   is a function, not a `const`, because this pulldown-cmark `bitflags!` version's
   `BitOr` isn't `const`. Not a spec deviation, but a place where "the obvious
   idiomatic code" wasn't available.

3. **The license** — the one thing I asked you (one question, as allowed): Apache-2.0.

Notably, *not* deviating was the default. The spec's completeness meant most
decisions were already made; the interesting work was faithful execution, not
design.

## The golden fixtures did the heavy lifting

The pre-shipped fixtures (`example.md` + expected `toc.{md,tsv,json}` + four range
outputs) were the single most valuable artifact. They are ground truth: I could
build the entire TOC engine and then assert byte-identical output, rather than
independently reasoning about what "correct" should be and possibly reasoning wrong.

Two things I did because of them:
- **Verified every SHA-256 against the spec up front** (all matched). Cheap, and it
  confirmed I was working against the intended inputs before writing a line of logic.
- **Wrote the golden-comparison integration tests early** (right after the TOC
  engine), so the first real "does this actually work end to end" check came fast.
  The gate "md/tsv/json byte-identical to `expected_toc.*`" is the single most
  reassuring result in the whole project.

## The API probe was the highest-leverage early step

Before writing real code I wrote a throwaway `examples/probe.rs` that fed a sample
document through pulldown-cmark's `into_offset_iter()` and printed the actual
`Event`/`Range` stream. That one step resolved several "I hope this is the shape I
think" questions:
- `into_offset_iter()` yields `(Event, Range)` with the *event first*.
- `Tag::Heading { level, .. }` — where the level lives.
- Footnote references emit `FootnoteReference` **only** when a definition exists
  (otherwise the text is literal `Text`) — so title handling is simpler than it
  looks.
- Heading attributes are stripped from the title text.
- YAML metadata blocks are *consumed*, so the "front matter becomes a Setext
  `title:` heading" trap **doesn't fire** with metadata-blocks enabled.

If I'd skipped the probe and guessed, at least two of those (footnote behavior,
the Setext trap) would have been wrong. **Lesson: when correctness depends on a
third-party library's exact event stream, probe it first and write down what you
see.** (I deleted the probe before finishing; it was scaffolding, not a deliverable.)

## Work breakdown: phases with measurable gates

The `impl-plan.md` broke the work into 6 phases, each ending in a **measurable
gate** ("line model: 15 unit tests green", "range parser: exact §9 messages",
"TOC: byte-identical to golden", "CLI: every error template + exit code + EPIPE",
"tests: invariants + fuzz", "polish: `cargo package` passes").

This was the right shape for two reasons:
- **Each gate made "done" unambiguous.** No phase ended on a feeling; it ended on
  a check. That made every commit a real checkpoint rather than a "somewhere in the
  middle" snapshot.
- **It made context compaction safe.** I compacted between phases (per the plan to
  keep context low), and the progress log — which recorded each gate's result — was
  the thing I re-read to re-orient. The log was deliberately written as "append as
  you go; re-read this if you get confused." It earned its keep.

**No subagents.** I did this inline, single-threaded, because the phases were
genuinely sequential — you can't build the TOC engine before the line model, and
range validation needs the line count. At ~1,700 lines total, the coordination
cost of fan-out would have exceeded the benefit. If this were a bigger, more
parallel surface (e.g. five independent subcommands), I'd reconsider; for a linear
pipeline of small modules, inline was faster.

## Things I'd do differently

- **Confirm the toolchain is actually usable before the first build.** The Rust
  toolchain wasn't on PATH (an earlier one-shot install had been interrupted), and
  one check got wrapped in a polling loop that burned 180 s and timed out. You
  called out "keep timeouts short" — that feedback was spot on. The fix is trivial:
  run the installer to completion, then run `cargo --version` **once**. No sleep/
  poll loops waiting for an install.
- **Verify platform behavior empirically, not from memory.** The one genuinely
  uncertain technical point was `EPIPE`: does Rust's std surface a `BrokenPipe`
  error (so I can exit 0 cleanly) or does the process die to `SIGPIPE`? I didn't
  assume — I piped a large output to `head` and checked the exit code. That's the
  pattern for anything touching signals/files/OS error text: tiny empirical repro,
  then trust it.
- **Default to private-access test helpers, not new public API.** The one
  ponytail-review finding (`heading_spans`) was a `pub fn` whose only callers were
  tests. The leaner design is a test that lives in the *same* module and calls the
  private function directly. I added the pub helper first because it felt
  convenient; the review caught it. Next time, "this is only for a test?" → put
  the test next to the code under test.
- **Don't chain a check-that-can-fail into the thing I'm measuring.** `cargo fmt
  --check && <perf timing>` — when the fmt check had nits, the `&&` stopped the
  chain and I silently didn't get the perf numbers. Run gates and measurements as
  separate commands.

## What went smoothly

- **The test layering** paid off. Unit tests pin the algorithms (line math, range
  grammar); golden integration tests prove end-to-end byte-identity; binary tests
  (`assert_cmd`) pin exit codes, exact stderr, and edge behaviors (BOM, bad UTF-8,
  `EPIPE`, `/dev/full`); invariant + fuzz tests guard the properties. Each layer
  catches a different failure class. For a *this* size tool it's on the heavy side,
  but §12 explicitly mandated most of it, so it wasn't gold-plating.
- **Running `test` + `clippy -- -D warnings` + `fmt` after every change** caught a
  handful of small slips I'd otherwise have shipped (a `é` in a `b""` literal, a
  `{line}`/`{end}` format-string typo, an edit-indentation mismatch, one garbled
  line I introduced while rewriting a file). Individually minor; collectively the
  reason the tree never accumulated cruft.
- **Verifying my own output rather than trusting the runner.** I diffed the
  binary's real stdout against the fixtures by hand for the load-bearing cases
  (`1-` reproducing the file, the exact stderr strings) rather than only trusting
  "21 passed." Cheap, and it's what actually convinced me the error strings were
  byte-correct.

## Takeaways to internalize

1. **Probe a third-party library's exact behavior before building on it**, and write
   down what you see. One throwaway script here removed the riskiest assumptions.
2. **Golden fixtures + hashes are ground truth.** Verify hashes first; build the
   tests around them; the "byte-identical to golden" check is the strongest
   end-to-end signal you can get.
3. **Give every phase a measurable gate.** It makes "done" objective and makes
   committing + compacting between phases safe.
4. **Log spec ambiguities + the judgment you made, the moment you find them** (the
   §5.2 example). A discrepancy noticed-and-noted is a footnote; one noticed-and-
   forgotten is a debugging session.
5. **Verify platform/OS behavior with a tiny repro** (signals, file errors), don't
   reason it from memory.
6. **Prefer a private test helper (same-file test module) over new public API** when
   the helper exists only to serve a test.
7. **Confirm the toolchain is actually on PATH and usable before the first build**,
   with a single version check — no polling loops, short timeouts.
8. **Run an over-engineering review at the end.** It's cheap, and it catches the
   slow drift where a "temporary test helper" quietly becomes part of the API.
9. **Keep a running progress log** that doubles as a re-orientation anchor for
   compaction and for anyone picking the work up later.
