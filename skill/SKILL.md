---
name: skimmd
description: "Use BEFORE reading any local Markdown file (.md, .markdown) from disk. `skimmd` prints a heading table of contents with line numbers and section sizes, then prints only the line ranges you need — skim first, fetch on demand, instead of loading the whole file into context. Trigger on any read of a Markdown file on disk (README, CHANGELOG, docs, spec, notes, book chapters, LLM outputs, transcripts), and whenever you would otherwise `cat`, `head`, `tail`, or open a `.md` file. Skip only if the content is already inline in your context or the file isn't Markdown."
---

# skimmd — skim Markdown at agent speed

## Why this exists

`skimmd` (<https://github.com/cwilper/skimmd>) works in two moves:

1. **See the structure.** `skimmd FILE` prints the file's headings as a
   table of contents with line numbers and per-section sizes.
2. **Fetch only what you need.** `skimmd FILE 12-30` prints exactly those
   lines, byte-for-byte, and nothing else.

## When to use it (and when not to)

Use skimmd BEFORE reading any `.md` or `.markdown` file from disk. The
default is "TOC first, then targeted range reads." Skip it only when:

- The file's content is already inline in your context (you were shown it
  by the user, or by an earlier tool result). Do not re-read.
- You already know from `wc -l` that the file is trivially small (say,
  <100 lines) AND the task requires essentially all of it. Even then, if
  the file has clear structure and you only need part of it, the TOC is
  still faster than eyeballing a full cat.

## Workflow

Assume `skimmd` is already installed and on your `PATH`. If it isn't,
say so — don't try to install it.

### Step 1 — Print the TOC

```bash
skimmd path/to/file.md
```

You get a Markdown table like this (one row per section):

```
| line | level | end | chars | title |
|---|---|---|---|---|
| 1   | 0 | 7   | 74   | preamble |
| 8   | 1 | 30  | 17   | Project |
| 12  | 2 | 19  | 23   | Install |
| 16  | 3 | 19  | 14   | macOS |
| 20  | 2 | 23  | 17   | Usage Notes |
```

How to read it:

- `line` — line the section's heading starts on. The leading preamble
  (anything before the first heading) is level 0 and titled `preamble`.
- `level` — heading depth (1–6); preamble is 0.
- `end` — the last line of the section's subtree (its heading, its body,
  and any deeper subsections up to the next heading of equal or
  shallower level).
- `chars` — character count of this section's body: everything after its
  own heading line and before the next heading of any level. Use it as a
  size hint: 0 means an empty section, tens of thousands means a section
  you almost certainly do not want to slurp whole.
- `title` — heading text with Markdown emphasis/code/links stripped and
  whitespace collapsed.

### Step 2 — Fetch just the lines you need

Pick the sections that match the task and pass their line ranges as
positional arguments:

```bash
# One range — the "Install" section (line 12 through end-of-section at 19)
skimmd path/to/file.md 12-19

# Multiple ranges — install + usage
skimmd path/to/file.md 12-19 20-23

# Comma-separated works too, and orders/overlaps are handled
skimmd path/to/file.md 12-19,20-23

# Open-ended range — from line 100 to end of file
skimmd path/to/file.md 100-
```

Notes on ranges:

- Both endpoints are **inclusive**.
- Ranges may be separated by commas, spaces, or both. Overlapping ranges
  merge; output is always in ascending line order.
- `skimmd FILE 1-` reproduces the file exactly (byte-identical after any
  UTF-8 BOM is dropped).

### Step 3 — Iterate if needed

If the section you fetched refers to another (e.g., "see Configuration
below"), go back to the TOC, find that section's line range, and fetch
it.

## Handy patterns

**Piped input:** `-` as the file reads stdin — `cat file.md | skimmd -` (TOC)
or `cat file.md | skimmd - 12-19` (range).

**Find a section by keyword without reading it:**

```bash
skimmd file.md | rg -i 'installation|setup'
# → jump directly to the matching line range
```

## Exit codes & errors

- `0` — success (also for a broken pipe when piping into `head`).
- `1` — request could not be satisfied: file missing, is a directory,
  not valid UTF-8, or an invalid range.
- `2` — usage error (bad flag, unknown `--format`).

Errors go to stderr as `skimmd: <message>`; on error, stdout is empty.

## Behavior worth knowing

- **YAML front matter** at the top of the file is treated as body, not a
  heading — so a `---`-fenced frontmatter block lives inside the level-0
  `preamble` row.
- **Setext headings** (`Title` then `===` or `---` underline) and **ATX
  headings** (`#`, `##`, ...) both count.
- **Headings inside code blocks, blockquotes, or list items are ignored**
  — they do not become TOC entries. That's usually what you want.
- **Line numbers are 1-based** over the raw file lines, so ranges you
  hand back to `sed`, `awk`, editors, or humans line up.
- **`FILE` is optional** — omit it (or pass `-`) to read the Markdown from
  standard input: `cat doc.md | skimmd` or `skimmd < doc.md`.
