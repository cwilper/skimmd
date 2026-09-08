These files show an example use of `skimmd` where the agent has
a large (>260k) article on Rain, and is seeking information about
acid rain and pollution specifically.

By getting the filtered TOC first, they were able to get the information
wanted with 1/20th of the tokens (`<13k`).

* [1.full-article.md](1.full-article.md) the original article on Rain,
  downloaded from Wikipedia and converted to Markdown.
* [2.full-toc.md](2.full-toc.md) table of contents only (`skimmd
  1.full-article.md`)
* [3.filtered-toc.md](3.filtered-toc.md) table of contents filtered by
  substrings (`skimmd 1.full-article.md -f "acid|pollution"`)
* [4.extracted-content.md](4.extracted-content.md) sections of interest
  extracted from the original article (`skimmd 1.full-article.md
  445-472,518-534,589-630`)

## Attribution & License

The original content is [the Wikipedia article for
Rain](https://en.wikipedia.org/wiki/Rain), retrieved
September 8th, 2026. It is available under the
[Creative Commons Attribution-ShareAlike License 4.0 (CC BY-SA
4.0)](https://creativecommons.org/licenses/by-sa/4.0/).

Files 2–4 are derived works (TOC, filtered TOC, and extracted
sections), and therefore are released under the same license.