---
name: skimmd
description: "Use before reading any local Markdown file (.md, .markdown) from disk — whenever you would otherwise `cat`, `head`, `tail`, or open one. `skimmd` prints a heading TOC with line numbers and section sizes, then only the line ranges you ask for, so the whole file never enters your context. Skip only if the content is already in your context, the file is not Markdown, or it is under ~100 lines and you need all of it."
---
# skimmd

For Markdown files bigger than you need in full: reading them whole wastes context and tokens, and `grep`/`rg` are too blunt to pull sections at the right granularity. So: TOC first (optionally filtered), then fetch only the lines you need. Assume it is on `PATH`; if the command is missing, say so — do not install it.

## 1. Find the sections you need

```bash
skimmd FILE                 # TOC of every section
skimmd FILE -f 'a|b'        # only rows whose title or body matches a|b (OR)
```

Example: a 260KB article on rain, you need the pollution-related parts. Cast a wide net first — matches are case-insensitive substrings, and `|` ORs the terms:

```
$ skimmd rain.md -f "acid|pollut"
| line | level | end | chars | title |
|---|---|---|---|---|
| 128 | 2 | 199 | 3152 | Contents |
| 445 | 3 | 472 | 5083 | Human influence |
| 518 | 3 | 534 | 1979 | Acidity |
| 589 | 3 | 630 | 5255 | Pollution and composition |
| 1121 | 2 | 2383 | 126347 | References |
```

One row per section, plus a level-0 `preamble` row for anything before the first heading (YAML front matter lives there).

| column | meaning |
|---|---|
| `line` | heading line (1-based; matches `grep -n` / `sed -n`) |
| `level` | heading depth 1–6; preamble = 0 |
| `end` | last line of the section **subtree** (heading + body + all subsections) |
| `chars` | size of the section's **own body** only (up to the next heading of any level); 0 = empty |
| `title` | heading text, markup stripped |

## 2. Fetch just the sections you want

The _Contents_ and _References_ rows above are irrelevant — fetch only what matters, in a single command:

```bash
skimmd rain.md 445-472,518-534,589-630   # ~12KB out of 260KB, ~1/20th
```

- Range `N-M` is verbatim lines, inclusive; comma- or space-separated; `N-` runs to EOF.
- Section **with** subsections → `line`–`end`. Section's **own text only** → `line` to the next row's `line` − 1.
- Use `chars` to size fetches: a large one means fetch a subrange instead of the section whole.
- `1-` is the whole file — only if you truly need all of it.

## Error codes

| code | meaning | do |
|---|---|---|
| 0 | ok (also on broken pipe) | — |
| 1 | request unsatisfiable: file missing / directory / not UTF-8 / bad range | fix the request, retry |
| 2 | usage error: bad flag | fix the invocation |

On 1 or 2 stdout is empty and stderr has `skimmd: <msg>`
