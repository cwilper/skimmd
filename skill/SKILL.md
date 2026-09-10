---
name: skimmd
description: "Use before reading any local Markdown file (.md, .markdown) from disk — whenever you would otherwise `cat`, `head`, `tail`, or open one. `skimmd` prints a heading TOC with line numbers and section sizes, then only the line ranges you ask for, so the whole file never enters your context. Skip only if the content is already in your context, the file is not Markdown, or it is under ~100 lines and you need all of it."
---
# skimmd

For Markdown files bigger than you need in full: reading them whole wastes context and tokens, and `grep`/`rg` are too blunt to pull sections at the right granularity. So: TOC first (optionally filtered), then fetch only the lines you need. Assume it is on `PATH`; if the command is missing, say so — do not install it.

## 1. Find the sections you need

```bash
skimmd FILE                 # TOC of every section
skimmd FILE -f 'a|b'        # only rows whose title or body matches a|b (OR), with a matches count
skimmd -                    # or: skimmd < FILE — read from stdin
```

Example: a 260KB article on rain, you need the pollution-related parts. Cast a wide net first — matches are case-insensitive substrings, and `|` ORs the terms:

```
$ skimmd rain.md -f "acid|pollut"
| line | level | end | chars | elided | title | matches |
|---|---|---|---|---|---|---|
| 128 | 2 | 199 | 3152 | 0 | Contents | 2 |
| 445 | 3 | 472 | 5083 | 0 | Human influence | 3 |
| 518 | 3 | 534 | 1979 | 0 | Acidity | 12 |
| 589 | 3 | 630 | 5255 | 0 | Pollution and composition | 8 |
| 1121 | 2 | 2383 | 126347 | 0 | References | 7 |
```

One row per section, plus a level-0 `preamble` row for anything before the first heading (YAML front matter lives there).

| column | meaning |
|---|---|
| `line` | heading line (1-based; matches `grep -n` / `sed -n`) |
| `level` | heading depth 1–6; preamble = 0 |
| `end` | last line of the section **subtree** (heading + body + all subsections) |
| `chars` | size of the section's **own body** only (up to the next heading of any level), after data-URL elision — what a default fetch returns; 0 = empty |
| `elided` | chars the elision removed from that body (`raw − chars`); `0` when the body has no elided data-URL payloads — a large value marks an image-heavy section (pre-elision total is `chars + elided`) |
| `title` | heading text, markup stripped |
| `matches` | **only with `-f`**: total occurrences of your candidate substrings in the section (heading + body, summed over candidates) |

Read `matches` alongside `chars` to rank hits: a small section with many matches is dense on-topic; a huge one with a few only brushes the topic. That's how `-f` gets you from a 260KB article to the ~12KB that matters.

## 2. Fetch just the sections you want

The _Contents_ and _References_ rows above are irrelevant — fetch only what matters, in a single command:

```bash
skimmd rain.md 445-472,518-534,589-630   # ~12KB out of 260KB, ~1/20th
```

- Range `N-M` is line-exact, inclusive (data-URL image payloads elided by default; `--raw` for verbatim); comma- or space-separated; `N-` runs to EOF.
- Section **with** subsections → `line`–`end`. Section's **own text only** → `line` to the next row's `line` − 1.
- Use `chars` to size fetches: a large one means fetch a subrange instead of the section whole.
- Range output elides data-URL image payloads to `data:…` (markdown form wrapped in `<!-- -->`); the elision is visible in the output. `--raw` emits lines verbatim.
- `1-` is the whole file — only if you truly need all of it.

## Error codes

| code | meaning | do |
|---|---|---|
| 0 | ok (also on broken pipe) | — |
| 1 | request unsatisfiable: file missing / directory / not UTF-8 / bad range | fix the request, retry |
| 2 | usage error: bad flag | fix the invocation |

On 1 or 2 stdout is empty and stderr has `skimmd: <msg>`
