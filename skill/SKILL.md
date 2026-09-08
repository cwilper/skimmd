---
name: skimmd
description: "Use before reading any local Markdown file (.md, .markdown) from disk — whenever you would otherwise `cat`, `head`, `tail`, or open one. `skimmd` prints a heading TOC with line numbers and section sizes, then only the line ranges you ask for, so the whole file never enters your context. Skip only if the content is already in your context, the file is not Markdown, or it is under ~100 lines and you need all of it."
---
# skimmd

Navigate one Markdown file without loading it all: TOC first, then fetch only the lines you need. Assume it is on `PATH`; if the command is missing, say so — do not install it.

## 1. TOC

```bash
skimmd FILE               # or: skimmd -  /  skimmd < FILE  (stdin)
skimmd FILE -f KEYWORD    # keep only rows whose heading OR body contains KEYWORD
```

One row per section, plus a level-0 `preamble` row for anything before the first heading (YAML front matter lives there):

```
| line | level | end | chars | title |
|---|---|---|---|---|
| 1   | 0 | 7   | 74   | preamble |
| 8   | 1 | 30  | 17   | Project |
| 12  | 2 | 19  | 23   | Install |
| 16  | 3 | 19  | 14   | macOS |
| 20  | 2 | 23  | 17   | Usage Notes |
```

| column | meaning |
|---|---|
| `line` | line the heading is on (1-based; matches `grep -n` / `sed -n`) |
| `level` | heading depth 1–6; preamble = 0 |
| `end` | last line of the section **subtree** (heading + body + all deeper subsections) |
| `chars` | size of the section's **own body** only (up to the next heading of any level); 0 = empty |
| `title` | heading text, markup stripped |

Range to fetch from a row:
- Section **with** its subsections → `line-end` (Install + macOS: `12-19`).
- Section's **own text only** → `line` to next row's `line` − 1 (Install alone: `12-15`).

Use `chars` to size fetches: a large `chars` means narrow further instead of fetching it whole.

`-f KEYWORD` is a case- and whitespace-insensitive plain substring (runs of whitespace, incl. newlines, collapse to one space), matched against each section's heading and body; TOC mode only. Prefer it over scanning a long TOC. No match → header-only table, exit 0.

## 2. Fetch

```bash
skimmd FILE 12-15 20-23   # verbatim lines; N-M inclusive, N- = to EOF; comma or space-separated
```

Any range switches to range mode. Overlaps merge; output is ascending. `1-` is the whole file — only if you truly need all of it.

## Error codes

| code | meaning | do |
|---|---|---|
| 0 | ok (also on broken pipe) | — |
| 1 | request unsatisfiable: file missing / directory / not UTF-8 / bad range | fix the request, retry |
| 2 | usage error: bad flag | fix the invocation |

On 1 or 2 stdout is empty and stderr has `skimmd: <msg>`
