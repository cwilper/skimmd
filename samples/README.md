These files show two example uses of `skimmd`.

## Set 1 — a large article (the main example)

An agent has a large (>260KB) article on Rain and is seeking information
about acid rain and pollution specifically.

By getting the filtered TOC first, they were able to get the information
wanted with 1/20th of the tokens (`<13k`).

This is the example the top-level README and `skill/SKILL.md` are built on.

* [1.full-article.md](1.full-article.md) the original article on Rain,
  downloaded from Wikipedia and converted to Markdown.
* [2.full-toc.md](2.full-toc.md) table of contents only (`skimmd
  1.full-article.md`)
* [3.filtered-toc.md](3.filtered-toc.md) table of contents filtered by
  substrings (`skimmd 1.full-article.md -f "acid|pollution"`)
* [4.extracted-content.md](4.extracted-content.md) sections of interest
  extracted from the original article (`skimmd 1.full-article.md
  445-472,518-534,589-630`)

## Set 2 — data-URL elision and the size columns

A small, original note-app export demonstrating the default data-URL elision,
`--raw`, and the `chars` / `elided` TOC columns: markdown base64 images
(elided, wrapped in `<!-- -->`), HTML `<img src="data:…">` attributes (elided),
a data URL in a fenced code block (kept verbatim), one in an inline code span
(elided, no wrap), a normal URL image (untouched), and a data-URL image with a
link title (payload elided, title kept).

* [5.data-images.md](5.data-images.md) the source document.
* [6.data-images-toc.md](6.data-images-toc.md) its table of contents (`skimmd
  5.data-images.md`) — image-heavy sections show a large `elided`; the
  code-block and text-only sections show `elided = 0`.
* [7.data-images-extracted.md](7.data-images-extracted.md) the image-bearing
  sections in default (elided) mode (`skimmd 5.data-images.md
  5-20,30-41,42-50`).

## Captured outputs

Files 2–4 and 6–7 are captured outputs; `sh samples/regenerate.sh` (from the
repo root) regenerates them and diffs against the committed copies.

## Attribution & License

The original content in Set 1 is [the Wikipedia article for
Rain](https://en.wikipedia.org/wiki/Rain), retrieved
September 8th, 2026. It is available under the
[Creative Commons Attribution-ShareAlike License 4.0 (CC BY-SA
4.0)](https://creativecommons.org/licenses/by-sa/4.0/).

Files 2–4 are derived works (TOC, filtered TOC, and extracted
sections), and therefore are released under the same license. Set 2 (files
5–7) is original content created for this repository.
