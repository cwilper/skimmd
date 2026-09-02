# skimmd — v1 Specification

`skimmd` is a small Rust command-line utility for agent-driven navigation of a single Markdown file. It has exactly two modes:

1. **TOC mode** — `skimmd FILE` prints a table of the file's structure (preamble, headings) with line numbers and sizes.
2. **Range mode** — `skimmd FILE RANGE...` prints the raw bytes of the requested line ranges, verbatim.

An agent calls mode 1 to decide what to read, then mode 2 to read it. The tool is deliberately minimal; it is intended to be wrapped later by an agent skill and/or an MCP server.

This document is the complete v1 contract.

---

## 1. Contents

2. CLI surface
3. Input handling and the line model
4. Front matter — parsed, not addressed
5. Heading extraction
6. TOC row computation
7. TOC output formats
8. Range mode
9. Errors and exit codes
10. Edge-case catalogue
11. Golden fixture and expected outputs
12. Test plan
13. Suggested layout and dependencies
14. Out of scope for v1

---

## 2. CLI surface

```
skimmd [--format <md|tsv|json>] <FILE> [RANGE]...
skimmd --help | -h
skimmd --version | -V
```

- `FILE` — path to a Markdown file. **The first positional argument is always the file.** Required.
- `RANGE...` — zero or more range specifications (see §9). All positionals after the first are ranges.
- `--format` — TOC output format. Default `md`. **Ignored in range mode** (do not error; agents may keep it in a command template).
- Flags may appear before or after positionals (clap default). Ranges never begin with `-`, so they cannot collide with flags.
- Use `clap` v4 with the derive API. Let clap own `--help`/`--version`.

Help text must include the range grammar (`N-M` or `N-`, comma- and/or space-separated, `N-` means through the last line) in one or two lines, since agents will read `--help`.

---

## 3. Input handling and the line model

### 3.1 Reading

1. Read the entire file into memory as bytes.
2. If the bytes begin with the UTF-8 BOM (`EF BB BF`), strip it. The BOM is never counted, never output, and never passed to the parser.
3. Decode as UTF-8. On failure: exit 1 with `skimmd: FILE: not valid UTF-8`. No lossy decoding in v1.
4. All subsequent processing operates on this decoded, BOM-stripped string, called **`text`**.

### 3.2 Lines

Lines are delimited by `\n` only.

- `\r\n` needs no special handling: the `\r` is simply the last character of its line. It is output verbatim and counted in `chars`.
- A lone `\r` is an ordinary character, not a line terminator. (Classic-Mac line endings are unsupported; document, don't handle.)
- **Line count** `N` = number of `\n` in `text`, plus 1 if `text` is non-empty and does not end with `\n`. An empty `text` has `N = 0`.
- **Line `i`** (1 ≤ i ≤ N) is the substring from `starts[i-1]` up to but not including `starts[i]`, where `starts[N]` is defined as `text.len()`. A line **includes its own `\n`** if present.

Build a line-start table once:

```
starts = [0]
for (j, b) in text.bytes().enumerate():
    if b == b'\n' and j + 1 < text.len():
        starts.push(j + 1)
N = if text.is_empty() { 0 } else { starts.len() }
```

Helpers (all 1-based, inclusive):

- `line_of(byte_offset)` → the largest `i` such that `starts[i-1] <= byte_offset` (binary search / `partition_point`). Precondition: `N > 0` and `byte_offset < text.len()`; callers must only invoke it with valid offsets.
- `span(a, b)` → `&text[starts[a-1] .. end_of(b)]` where `end_of(b) = if b < N { starts[b] } else { text.len() }`. Returns `""` when `a > b`.
- `chars(a, b)` → `span(a, b).chars().count()`.

Line numbers map 1:1 to what `grep -n`, `sed -n 'a,bp'`, and editors report. This is a hard requirement.

---

## 4. Front matter — parsed, not addressed

Front matter (YAML `---` blocks) is **not** a separately addressable region in skimmd. Its lines belong to whatever TOC region covers them — in practice, the `preamble` row (§6.1). An agent that needs only the YAML block reads the preamble and extracts the fenced block itself.

### 4.1 Parser configuration for metadata blocks

Do **not** write a manual front-matter scanner. Parse `text` with `Options::ENABLE_YAML_STYLE_METADATA_BLOCKS` enabled so the parser consumes metadata blocks instead of misreading them. The `MetadataBlock` events themselves are **ignored** by skimmd; the option is enabled purely so that:

- A top-of-file `---\ntitle: Foo\n---` is not parsed as a thematic break followed by a **Setext H2 titled "title: Foo"**.
- A mid-document `---\nfoo\n---` is not parsed as a Setext heading. This is accepted behavior; note it in the README.

Do **not** enable `ENABLE_PLUSES_DELIMITED_METADATA_BLOCKS` in v1 (TOML front matter is out of scope).

---

## 5. Heading extraction

### 5.1 Parser configuration

```rust
let opts = Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
         | Options::ENABLE_HEADING_ATTRIBUTES
         | Options::ENABLE_STRIKETHROUGH
         | Options::ENABLE_FOOTNOTES;
let parser = Parser::new_ext(&text, opts).into_offset_iter();
```

- `ENABLE_HEADING_ATTRIBUTES` makes `## Foo {#id .cls}` yield title `Foo` (attributes parsed out of the text). Side effect: a heading whose text legitimately ends in `{...}` will lose that suffix. Accepted.
- `ENABLE_STRIKETHROUGH` / `ENABLE_FOOTNOTES` change how inline content *inside a heading* renders (e.g. `~~done~~` → `done`, `[^1]` dropped); keep them — they are part of the §5.2 title contract.
- **Never** enable `ENABLE_SMART_PUNCTUATION` — it rewrites quotes and dashes in rendered text, making titles diverge from the source.
- `ENABLE_MATH`, `ENABLE_GFM`, and others are not needed; leave off.

### 5.2 Collecting headings

Iterate the offset iterator. Verified from the 0.13 source (`parse.rs::OffsetIter::next`): both `Event::Start` and `Event::End` of a block carry the block's **full** byte range `item.start..item.end`.

On `Event::Start(Tag::Heading { level, .. })` with range `r`:

1. `first_line = line_of(r.start)`
2. `last_line = line_of(r.end.saturating_sub(1))`
   - ATX: `r` covers the `#…` line through its EOL (source: `parse_atx_heading` sets `end = ix + eol_bytes`). `first_line == last_line`.
   - Setext: `r` covers the content line(s) **through the underline line** (source: `parse_setext_heading` returns `ix + n` past the underline; the node is popped at that offset). `last_line` is the underline line. Multi-line Setext content is possible; `first_line < last_line` then.
3. `level = level as u8` (`HeadingLevel::H1..H6` → 1..6).
4. Collect the title from the inline events until the matching `Event::End(TagEnd::Heading(_))`:
   - `Event::Text(s)` → append `s`
   - `Event::Code(s)` → append `s` (no backticks)
   - `Event::SoftBreak | Event::HardBreak` → append a single space
   - `Event::InlineMath(s) | Event::DisplayMath(s)` → append `s`
   - `Event::Start(_) / Event::End(_)` for inline tags (Emphasis, Strong, Strikethrough, Link, Image) → append nothing; their inner `Text` events still arrive and are kept. Link URLs and image sources are therefore dropped; link text and image alt text are kept.
   - `Event::Html(_) | Event::InlineHtml(_) | Event::FootnoteReference(_)` → append nothing
5. Normalize the title: trim leading/trailing whitespace; collapse every internal run of whitespace (including any `\r`) to a single space. Titles therefore never contain newlines or tabs.

Because tag boundaries contribute nothing, adjacent inlines concatenate without a separator: `## a *b*c* d` yields the title `abc d`. That is the intended behavior.

Record `(first_line, last_line, level, title)`. Headings arrive in document order; keep that order.

Headings inside a `MetadataBlock` cannot occur (the block body is text). Headings inside fenced/indented code blocks are not emitted by the parser. Headings inside blockquotes or list items **are** emitted by the parser and **are** included, with `level` equal to their `#` count regardless of container nesting. HTML `<h2>` is an `Html` event, not a heading, and is not included.

An empty ATX heading (`#` alone on a line) is valid CommonMark and yields a row with an empty title.

---

## 6. TOC row computation

Inputs: `N` and the heading list `H[0..k)`.

Every row has exactly five fields: `line`, `level`, `end`, `chars`, `title`.

### 6.1 Preamble row (only if `N > 0` and line 1 is not a heading)

Emit the row iff `N > 0` and (`k == 0` or `H[0].first_line > 1`). The region is lines `1 ..= P` where `P = H[0].first_line - 1` if `k > 0`, else `P = N`.

```
line  = 1
level = 0
end   = P
chars = chars(1, P)          // includes any front-matter fence lines
title = "preamble"
```

The region contains everything before the first heading: any front matter, any introductory text, and any other leading content (thematic breaks, HTML, blank lines). It is emitted even when the region consists solely of blank lines — `chars` then simply reflects the blank content. The title is the fixed sentinel `preamble`; the row is the only level-0 row and is always first.

### 6.2 Heading rows

For each heading `h = H[i]`:

```
line   = h.first_line
level  = h.level
body_a = h.last_line + 1
body_b = if i + 1 < k { H[i+1].first_line - 1 } else { N }
chars  = chars(body_a, body_b)         // 0 when body_a > body_b
end    = subtree_end(i)
title  = h.title
```

`subtree_end(i)` = `H[j].first_line - 1` for the smallest `j > i` with `H[j].level <= h.level`; if no such `j` exists, `N`.

Compute `end` in one forward pass with a stack: push each heading's index; when heading `j` arrives, pop every stacked index `i` with `H[i].level >= H[j].level` and set `end[i] = H[j].first_line - 1`. After the loop, every index still on the stack gets `end = N`.

Note the two different right-hand boundaries:

- `chars` stops at the **next heading of any level** (own body only).
- `end` stops before the **next heading of equal or shallower level** (whole subtree).

Guarantees the implementation can assert:

- `end >= line` for every row, so the range `line-end` is always valid. For a heading row, `line-end` reads the heading plus its entire subtree. For a heading with no body, `end == last_line`. For the preamble row, `line-end` reads exactly the pre-heading region.
- For every non-empty file, the **last** row's `end` equals `N`.
- To read a heading's own body without children, an agent uses `(line+1)-(nextrow.line-1)`; the tool does not emit a separate body-end column in v1.
- Accounting invariant (use as a test): `chars(1, P) + Σ chars(h.first_line, h.last_line) + Σ heading-row chars == text.chars().count()`, where the preamble term is `0` when the preamble row is suppressed (equivalently, `chars(1,P)` with `P = 0`).

### 6.3 Ordering

Rows are emitted in document order: preamble (if present), then headings by `line`.

---

## 7. TOC output formats

Column order in every format: `line`, `level`, `end`, `chars`, `title`. Integers are plain decimal. Output ends with exactly one `\n`.

### 7.1 `md` (default) — compact GitHub-style table

```
| line | level | end | chars | title |
|---|---|---|---|---|
| 1 | 0 | 7 | 74 | preamble |
| 8 | 1 | 30 | 17 | Project |
```

- Header row and alignment row always present, even when there are zero data rows.
- Cells are ` value ` (one space each side). **No column-width padding.**
- Title escaping: replace `|` with `\|`. Nothing else needs escaping (titles have no newlines/tabs after §5.2 normalization).
- An empty title renders as `|  |`.
- No header/footer metadata: `md` output is exactly the table and nothing else.

### 7.2 `tsv`

```
line	level	end	chars	title
1	0	7	74	preamble
```

- Header row always present.
- Fields separated by a single tab; no trailing tab; rows terminated by `\n`.
- Title escaping: none required (tabs/newlines cannot occur after §5.2 normalization).
- No header/footer metadata: `tsv` output is exactly the header plus data rows.

### 7.3 `json`

```
{"lines":30,"toc":[{"line":1,"level":0,"end":7,"chars":74,"title":"preamble"},{"line":8,"level":1,"end":30,"chars":17,"title":"Project"}]}
```

- Single line, compact (no whitespace), followed by `\n`.
- Top-level object with exactly two keys in this order: `lines` (= `N`) and `toc` (array of row objects with keys in the order `line, level, end, chars, title`).
- Standard JSON string escaping for `title` (`serde_json`). Do not escape non-ASCII.
- Empty file → `{"lines":0,"toc":[]}`.

`N` is exposed only via the `json` `lines` key. Agents that need the file length in `md`/`tsv` mode can use `--format json` or infer it from range-mode validation errors (`start … exceeds file length (N lines)`).

---

## 8. Range mode

### 8.1 Grammar

Each positional argument after `FILE` is a `SPEC`:

```
SPEC  := RANGE ( ',' RANGE )*
RANGE := INT '-' INT?
INT   := [0-9]+
```

- No whitespace anywhere inside a `SPEC`.
- Every `RANGE` contains exactly one `-`. A bare integer (`42`) is a syntax error.
- Leading zeros are permitted and ignored (`007-010` ≡ `7-10`).
- Multiple `SPEC` arguments are allowed; all ranges from all specs are pooled. `skimmd f.md 1-5,9-12` and `skimmd f.md 1-5 9-12` are equivalent.
- A `RANGE` with no right-hand integer (`N-`) means "through the last line of the file".
- There are no special or shorthand values. Every integer is a plain 1-based physical line number; the smallest valid start is `1`.

Parse every spec fully **before** producing any output; if any spec fails syntax or validation, emit nothing to stdout and exit 1.

### 8.2 Validation

Ranges are validated in pool order (arguments left to right, ranges within each argument left to right). The **first** failing range determines the error; validation stops there.

For each `RANGE` with literal `start` and optional literal `end`:

1. If `start < 1` → error `range START-END: start must be at least 1`
2. If `end` is present and `end < start` → error `range START-END: end is less than start`
3. If `start > N` → error `range START-END: start START exceeds file length (N lines)`
4. If `end` is present and `end > N` → error `range START-END: end END exceeds file length (N lines)`

After validation, every range maps to a non-empty effective interval `[start, eff_end]` where `eff_end = end if present else N`.

### 8.3 Normalization

1. Sort by effective start.
2. Merge: walking in order, if `next.start <= cur.end + 1`, extend `cur.end = max(cur.end, next.end)`; else emit `cur` and start a new one. (Merging adjacent ranges as well as overlapping ones is fine because output has no separators, so the bytes are identical either way.)

Each file line is output at most once, in ascending order.

### 8.4 Output

For each merged range `[a, b]`, write `span(a, b)` to stdout, in order, with **no** separators between ranges and **no** appended trailing newline. If the final output line is the file's last line and the file lacks a trailing `\n`, the output also lacks it. Output is byte-exact: `skimmd f.md 1-` must reproduce `text` exactly (i.e. the file minus any BOM).

Use a `BufWriter` on stdout and flush once. **Only** `EPIPE` (broken pipe) is treated as a clean exit 0 with no message. Any other stdout write error (e.g. disk full on a redirected output) is exit 1 with `skimmd: error writing to stdout: OS_ERROR`.

---

## 9. Errors and exit codes

| Exit | Meaning |
|---|---|
| 0 | Success |
| 1 | The request could not be satisfied: file not found / unreadable / is a directory / not UTF-8; range syntax error; range validation error; stdout write error (non-EPIPE) |
| 2 | Command-line usage error as produced by clap (missing `FILE`, unknown flag, bad `--format` value) |

All error messages go to stderr, one line, prefixed `skimmd: `. Exact templates (tests should match these):

```
skimmd: FILE: No such file or directory          (or the OS error text)
skimmd: FILE: is a directory
skimmd: FILE: not valid UTF-8
skimmd: invalid range spec 'SPEC': expected N-M or N- (comma-separated)
skimmd: range START-END: start must be at least 1
skimmd: range START-END: end is less than start
skimmd: range START-END: start START exceeds file length (N lines)
skimmd: range START-END: end END exceeds file length (N lines)
skimmd: error writing to stdout: OS_ERROR
```

In the range templates, `START-END` is the literal text of the single failing range exactly as the user wrote it (leading zeros preserved, e.g. `007-`), not the whole spec. The `START`/`END` placeholders in the body of the message are likewise the literal tokens as written. The reported range is the first failing one in pool order (§8.2).

Never write partial output before an error. Never panic on any input file (fuzz-test this).

---

## 10. Edge-case catalogue

| Case | Behavior |
|---|---|
| Empty file (0 bytes, or BOM only) | `N=0`. TOC: header rows only (`md`/`tsv`) or `{"lines":0,"toc":[]}`. Ranges: any range is an error (`start … exceeds file length (0 lines)`). |
| File is only front matter | One row (`preamble`), `end = N`. |
| File is only blank lines | `N > 0`, one `preamble` row spanning the whole file. |
| No headings, non-blank content | One `preamble` row spanning `1..N`. |
| File starts with a heading | No preamble row; first row is the heading at `line 1`. |
| Heading immediately followed by heading | First has `chars = 0`, `end = its last_line`. |
| Heading is the last line | `chars = 0` if nothing follows; `end = N`. |
| Level jumps (`#` then `###`) | `end` uses level comparison; works unchanged. |
| Multiple `#` H1s | Each is its own subtree root. |
| Setext heading | `line` = first content line; heading occupies content line(s) + underline; body starts after the underline. |
| Multi-line Setext content | Lines joined with a single space in the title. |
| `#` alone on a line | Row with empty title. |
| Trailing `###` in ATX (`## Foo ##`) | Parser strips it; title `Foo`. |
| `## Foo {#id}` | Title `Foo` (attributes enabled). |
| Heading inside ``` fence | Not a heading. |
| Heading inside `>` or list item | Is a heading; level = `#` count. |
| `<h2>` HTML | Not a heading. |
| `---\ntitle: x\n---` at top | Parsed as a metadata block: no false Setext heading; its lines belong to the `preamble` row. |
| `---\n\ntitle: x\n---` at top | **Not** a metadata block (blank first line). Parser then yields a Setext H2 "title: x"; TOC shows a `preamble` row (lines 1–2) plus the heading row. Known limitation; document. |
| `---\n---` at top | Not a metadata block; two thematic breaks; one `preamble` row spanning both lines. |
| Front matter closed with `...` | Consumed as a metadata block (parser rule); lines belong to the `preamble` row. |
| Front matter closed with `----` | Not closed; unterminated → thematic break; no metadata block. |
| CRLF file | `\r` stays on its line, is output verbatim, and is counted in `chars`. |
| No trailing newline | Last line counted; range output ends without `\n`. |
| UTF-8 BOM | Stripped; invisible to every mode. |
| Non-UTF-8 bytes | Exit 1. |
| Range `0-` or `0-0` | Error: `start must be at least 1` (no special values in v1). |
| Title contains `\|` | Escaped in `md`; raw in `tsv`; JSON-escaped in `json`. |
| Title contains non-ASCII | Passed through unescaped in all formats. |
| Very large file | Whole file in memory; fine for v1. |
| `FILE` named like a range (`10-20`) | First positional is always the file; it is opened as a path. |
| Broken pipe while writing | Exit 0, no message. Other stdout write errors: exit 1. |

---

## 11. Golden fixture and expected outputs

The fixture below is the primary integration test. It exercises front matter, preamble, H1/H2/H3, a `|` in a title, a Setext heading, an empty-body heading, and a final line with no trailing newline.

### 11.1 Create the fixture byte-exactly

Command substitution strips the trailing newline, which is intended:

```sh
printf '%s' "$(cat <<'EOF'
---
title: Example
tags: [a, b]
---

Intro paragraph before any heading.

# Project

Overview text.

## Install

Run `cargo install`.

### macOS

brew stuff.

## Usage | Notes

Body of usage.

Setext Heading
--------------

Body of setext.
## Empty
## Last
Final line without newline
EOF
)" > example.md
```

Verify: `wc -c example.md` → `283`; `sha256sum` → `d2ab3e767e66f85c7aeab61c15b4a2803446aa536976ca3510f4440d9aeab7d3`; `N = 30`.

Line map for reference: 1–4 front matter · 5–7 preamble text (1–7 together form the `preamble` row) · 8 `# Project` · 12 `## Install` · 16 `### macOS` · 20 `## Usage | Notes` · 24–25 Setext H2 · 28 `## Empty` · 29 `## Last` · 30 last line (no `\n`).

### 11.2 `skimmd example.md` (md, default)

```
| line | level | end | chars | title |
|---|---|---|---|---|
| 1 | 0 | 7 | 74 | preamble |
| 8 | 1 | 30 | 17 | Project |
| 12 | 2 | 19 | 23 | Install |
| 16 | 3 | 19 | 14 | macOS |
| 20 | 2 | 23 | 17 | Usage \| Notes |
| 24 | 2 | 27 | 17 | Setext Heading |
| 28 | 2 | 28 | 0 | Empty |
| 29 | 2 | 30 | 26 | Last |
```

### 11.3 `skimmd --format tsv example.md`

```
line	level	end	chars	title
1	0	7	74	preamble
8	1	30	17	Project
12	2	19	23	Install
16	3	19	14	macOS
20	2	23	17	Usage | Notes
24	2	27	17	Setext Heading
28	2	28	0	Empty
29	2	30	26	Last
```

### 11.4 `skimmd --format json example.md`

```
{"lines":30,"toc":[{"line":1,"level":0,"end":7,"chars":74,"title":"preamble"},{"line":8,"level":1,"end":30,"chars":17,"title":"Project"},{"line":12,"level":2,"end":19,"chars":23,"title":"Install"},{"line":16,"level":3,"end":19,"chars":14,"title":"macOS"},{"line":20,"level":2,"end":23,"chars":17,"title":"Usage | Notes"},{"line":24,"level":2,"end":27,"chars":17,"title":"Setext Heading"},{"line":28,"level":2,"end":28,"chars":0,"title":"Empty"},{"line":29,"level":2,"end":30,"chars":26,"title":"Last"}]}
```

Sanity checks on these numbers: the `preamble` row spans lines 1–7 (front matter 1–4 plus intro text 5–7: 36 + 38 = 74 chars). The `Install` subtree (`12-19`) ends just before `## Usage` at line 20; `macOS` shares that `end` because it is the last child. `Setext Heading` reports `line 24` (content line), and its body (`26-27`) starts after the underline at 25. `Empty` has `chars 0` and `end == line`. `Last`'s 26 chars are exactly `Final line without newline` with no terminator. The chars invariant holds: 74 (preamble) + 95 (heading lines: 10 + 11 + 10 + 17 + 30 + 9 + 8; the Setext heading spans its content line and underline) + 114 (heading bodies: 17 + 23 + 14 + 17 + 17 + 0 + 26) = 283.

### 11.5 Range mode

`skimmd example.md 12-19` → 58 bytes:

```
## Install

Run `cargo install`.

### macOS

brew stuff.

```
(ends with `\n`; the last line of the range, 19, is blank.)

`skimmd example.md 1-4` → 36 bytes, the front-matter block (read directly as a line range; no special value needed), ending with `\n`:

```
---
title: Example
tags: [a, b]
---
```

`skimmd example.md 8-8,16-` (equivalently `8-8 16-`) → 158 bytes, line 8 immediately followed by lines 16–30, **no** trailing newline:

```
# Project
### macOS

brew stuff.

## Usage | Notes

Body of usage.

Setext Heading
--------------

Body of setext.
## Empty
## Last
Final line without newline
```

`skimmd example.md 29-` → 34 bytes, no trailing newline:

```
## Last
Final line without newline
```

`skimmd example.md 1-` → byte-identical to `example.md`.

`skimmd example.md 5-10,3-6` → identical to `skimmd example.md 3-10` (merge).

Errors (exit 1, no stdout):

- `skimmd example.md 31-` → `skimmd: range 31-: start 31 exceeds file length (30 lines)`
- `skimmd example.md 10-5` → `skimmd: range 10-5: end is less than start`
- `skimmd example.md 5` → `skimmd: invalid range spec '5': expected N-M or N- (comma-separated)`
- `skimmd example.md 1-10,x-y` → `skimmd: invalid range spec '1-10,x-y': expected N-M or N- (comma-separated)`
- `skimmd example.md 0-` → `skimmd: range 0-: start must be at least 1`
- `skimmd example.md 5-10,0-2` → `skimmd: range 0-2: start must be at least 1` (first failing range in pool order)

---

## 12. Test plan

1. **Golden tests** — `tests/fixtures/example.md` plus expected files for each TOC format and each range case in §11 (including `expected_1-4.txt`). Compare bytes, not lines.
2. **Line-model unit tests** — `N`, `starts`, `span`, `line_of` on: empty; `"\n"`; `"a"`; `"a\n"`; `"a\nb"`; `"a\n\n"`; CRLF text; text with a lone `\r`.
3. **Range parser unit tests** — table-driven: each of `1-`, `1-1`, `007-010`, `1-5,9-12`, and each error form (`5`, `0-`, `0-0`, `10-5`, `1-10,x-y`, `1-` on an empty file); assert normalized output or error kind and message.
4. **Setext-trap test** — a file whose only content is `---\ntitle: Foo\n---\n\n# Real\n` must yield exactly two rows: `preamble` (1–4) and `Real` (line 5). No row titled `title: Foo`.
5. **Blockquote/list/code tests** — `> ## Quoted`, `- ## Listed`, and a `# Fenced` inside ``` fence: the first two are rows, the third is not.
6. **Heading-attribute test** — `## Foo {#foo .x}` → title `Foo`.
7. **Setext multi-line test** — `Alpha\nBeta\n=====\n` → one row, line 1, level 1, title `Alpha Beta`; body starts at line 4. Note: line 1 is a heading, so no preamble row; add a variant with a leading blank line to cover the preamble row (`\nAlpha\nBeta\n=====\n` → preamble row 1–1, heading row line 2).
8. **Invariant property test** — for every fixture: (a) accounting invariant from §6.2; (b) for every TOC row, `skimmd FILE line-end` succeeds; (c) `skimmd FILE 1-` reproduces the file bytes (post-BOM); (d) last row's `end == N` for every non-empty fixture.
9. **Fuzz** — feed random bytes / random UTF-8 through TOC mode and assert no panic (a `proptest` or `cargo-fuzz` harness is fine; even a loop over `/usr/share/dict` style inputs is useful).
10. **Exit-code tests** — nonexistent file, directory, invalid UTF-8, bad `--format`, missing `FILE`, and `0-` (exit 1, `start must be at least 1`).

---

## 13. Suggested layout and dependencies

```
Cargo.toml
src/
  main.rs        // clap definitions, mode dispatch, exit codes, stderr formatting
  lines.rs       // starts table, N, line_of, span, chars
  toc.rs         // parser setup, heading extraction, row computation
  ranges.rs      // SPEC grammar, validation, normalization
  format.rs      // md / tsv / json emitters
tests/
  fixtures/example.md and expected outputs
  cli.rs         // integration tests invoking the binary (assert_cmd is convenient)
```

```toml
[dependencies]
clap            = { version = "4", features = ["derive"] }
pulldown-cmark  = "0.13"
serde_json      = "1"

[dev-dependencies]
assert_cmd = "2"
```

Keep the crate free of a markdown *renderer*, HTTP, or async — nothing here needs them. Target: `< 1 s` for a 10 MB file; the parse is linear and everything else is table lookups.

---

## 14. Out of scope for v1 (leave room, don't build)

"Leave room, don't build" means: the architecture must keep these doors open (see the note below), and this spec may record that they were considered and deliberately deferred — but out-of-scope features must **never** be listed in implementation code or in official user-facing documentation (README, `--help` text, man pages).

- Link counts (inbound/outbound intra-document, external) as extra columns.
- Content hash in TOC output; caching; TTLs.
- Plain-text input with heuristic heading detection.
- Automatic sub-chunking of oversized sections at sentence boundaries.
- TOML (`+++`) or other front-matter syntaxes.
- Range shorthand or special values (the removed `0`-means-front-matter rule stays removed; if front-matter addressing ever becomes valuable, it should be an explicit opt-in flag, not an overload of a line number).
- `--separator` between ranges; `--depth` to limit TOC levels; a body-end column.
- MCP server or skill packaging. The CLI contract above is stable enough to wrap: a skill needs only "run `skimmd FILE`, choose rows, run `skimmd FILE line-end`".
- Maintenance constraint for anyone extending the CLI: **never introduce a flag that begins with a digit** (ranges are digit-led positional arguments and must stay unambiguous).

Column order is load-bearing: every format is columnar with `title` last, so any future numeric TOC column goes before `title`; the `json` `lines` key is the slot a future per-file value (e.g. `hash`) joins.
