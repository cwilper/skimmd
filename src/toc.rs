//! Heading extraction and TOC row computation.
//!
//! See spec §5 (heading extraction) and §6 (TOC row computation). Every row has
//! exactly five fields in this order: `line`, `level`, `end`, `chars`, `title`.
//!
//! Two distinct right-hand boundaries are computed per heading:
//! * `chars` stops at the **next heading of any level** (the heading's own body).
//! * `end` stops before the **next heading of equal or shallower level** (subtree).

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use serde::Serialize;

use crate::lines::LineMap;

/// One TOC row. Field order is load-bearing (matches every output format).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    pub line: usize,
    pub level: u8,
    pub end: usize,
    pub chars: usize,
    pub title: String,
}

/// A heading as extracted from the document.
#[derive(Debug)]
struct Heading {
    first_line: usize,
    last_line: usize,
    level: u8,
    title: String,
}

/// The exact parser options (spec §4.1 + §5.1). `ENABLE_SMART_PUNCTUATION` is
/// deliberately **off** (it would rewrite quotes/dashes and diverge titles).
/// (Not `const`: this `bitflags!` version's `BitOr` is non-const.)
fn parser_opts() -> Options {
    Options::ENABLE_YAML_STYLE_METADATA_BLOCKS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
}

/// Build the TOC rows for `lm`: a leading `preamble` row (if any) followed by one
/// row per heading, in document order.
#[must_use]
pub fn build_toc(lm: &LineMap) -> Vec<Row> {
    let text = lm.text();
    let n = lm.n();
    let headings = extract_headings(text, lm);
    let k = headings.len();

    let mut rows = Vec::with_capacity(k + 1);

    // Preamble row (§6.1): emitted iff N > 0 and line 1 is not a heading.
    if n > 0 && (k == 0 || headings[0].first_line > 1) {
        let p = if k > 0 { headings[0].first_line - 1 } else { n };
        rows.push(Row {
            line: 1,
            level: 0,
            end: p,
            chars: lm.chars(1, p),
            title: "preamble".into(),
        });
    }

    // subtree_end(i) via a stack (§6.2): pop while top.level >= cur.level.
    let mut ends = vec![n; k];
    let mut stack: Vec<usize> = Vec::new();
    for j in 0..k {
        while let Some(&i) = stack.last() {
            if headings[i].level >= headings[j].level {
                ends[i] = headings[j].first_line - 1;
                stack.pop();
            } else {
                break;
            }
        }
        stack.push(j);
    }
    // indices still on the stack keep `ends[i] = n` (their initial value).

    for (i, h) in headings.iter().enumerate() {
        let body_a = h.last_line + 1;
        let body_b = if i + 1 < k {
            headings[i + 1].first_line - 1
        } else {
            n
        };
        rows.push(Row {
            line: h.first_line,
            level: h.level,
            end: ends[i],
            chars: lm.chars(body_a, body_b),
            title: h.title.clone(),
        });
    }

    rows
}

/// Heading line-spans `(first_line, last_line)` in document order.
///
/// A verification helper (e.g. for the §6.2 accounting invariant): the TOC rows
/// themselves only expose `line` (== `first_line`). Re-parses; not a hot path.
#[must_use]
pub fn heading_spans(lm: &LineMap) -> Vec<(usize, usize)> {
    extract_headings(lm.text(), lm)
        .iter()
        .map(|h| (h.first_line, h.last_line))
        .collect()
}

/// Walk the offset iterator and collect headings in document order (§5.2).
fn extract_headings(text: &str, lm: &LineMap) -> Vec<Heading> {
    let mut headings = Vec::new();
    // While inside a heading: (first_line, last_line, level, title buffer).
    let mut cur: Option<(usize, usize, u8, String)> = None;

    for (ev, range) in Parser::new_ext(text, parser_opts()).into_offset_iter() {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                let first = lm.line_of(range.start);
                let last = lm.line_of(range.end.saturating_sub(1));
                cur = Some((first, last, level_to_u8(level), String::new()));
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((first, last, level, buf)) = cur.take() {
                    headings.push(Heading {
                        first_line: first,
                        last_line: last,
                        level,
                        title: normalize_title(&buf),
                    });
                }
            }
            ev if cur.is_some() => {
                let title = &mut cur.as_mut().expect("checked cur.is_some()").3;
                match ev {
                    // Keep: visible text, code (no backticks), math.
                    Event::Text(s)
                    | Event::Code(s)
                    | Event::InlineMath(s)
                    | Event::DisplayMath(s) => title.push_str(&s),
                    // Soft/hard breaks collapse to a single space.
                    Event::SoftBreak | Event::HardBreak => title.push(' '),
                    // Inline tags (Emphasis/Strong/Strikethrough/Link/Image):
                    // boundaries add nothing; their inner Text is kept above.
                    // Html/InlineHtml/FootnoteReference: dropped.
                    _ => {}
                }
            }
            // Block-level events outside a heading (paragraphs, metadata blocks,
            // code, lists, blockquotes, …): ignored.
            _ => {}
        }
    }

    headings
}

/// `HeadingLevel` → 1..=6. (Explicit match; the enum's repr is not relied on.)
fn level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Trim and collapse every internal whitespace run (incl. `\r`) to a single space.
/// Titles therefore never contain newlines or tabs (§5.2 step 5).
#[must_use]
fn normalize_title(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toc_of(s: &str) -> Vec<Row> {
        build_toc(&LineMap::new(s.to_string()))
    }

    fn rows_summary(rows: &[Row]) -> Vec<(usize, u8, usize, usize, &str)> {
        rows.iter()
            .map(|r| (r.line, r.level, r.end, r.chars, r.title.as_str()))
            .collect()
    }

    #[test]
    fn setext_trap_is_avoided() {
        // §12.4: front matter must not become a Setext "title: Foo" heading.
        let rows = toc_of("---\ntitle: Foo\n---\n\n# Real\n");
        let s = rows_summary(&rows);
        assert_eq!(
            s,
            vec![
                (1, 0, 4, 20, "preamble"), // lines 1-4
                (5, 1, 5, 0, "Real"),
            ],
            "no row titled 'title: Foo'"
        );
    }

    #[test]
    fn heading_attribute_is_stripped() {
        // §12.6: `## Foo {#foo .x}` -> title `Foo`.
        let rows = toc_of("## Foo {#foo .x}\nbody\n");
        assert_eq!(rows[0].title, "Foo");
        assert_eq!(rows[0].line, 1);
        assert_eq!(rows[0].level, 2);
    }

    #[test]
    fn blockquote_and_list_headings_count() {
        // §12.5: `> ## Quoted` and `- ## Listed` are headings (level = # count).
        let rows = toc_of("> ## Quoted\n- ## Listed\n");
        let s = rows_summary(&rows);
        assert_eq!(s[0], (1, 2, 1, 0, "Quoted"));
        assert_eq!(s[1], (2, 2, 2, 0, "Listed"));
    }

    #[test]
    fn fenced_heading_is_not_a_heading() {
        // §12.5: `# Fenced` inside a ``` fence is not a heading.
        let rows = toc_of("```text\n# Fenced\n```\n");
        assert_eq!(rows.len(), 1, "only the preamble row: {rows:?}");
        assert_eq!(rows[0].level, 0);
        assert_eq!(rows[0].title, "preamble");
    }

    #[test]
    fn setext_multiline_title() {
        // §12.7: `Alpha\nBeta\n=====` -> line 1, level 1, title `Alpha Beta`,
        // body starts at line 4.
        let rows = toc_of("Alpha\nBeta\n=====\n");
        let r = &rows[0];
        assert_eq!(r.line, 1);
        assert_eq!(r.level, 1);
        assert_eq!(r.title, "Alpha Beta");
        // heading spans lines 1-3 (content 1-2 + underline 3), body at 4..N=3? none
        assert_eq!(r.end, 3);
        assert_eq!(r.chars, 0);
    }

    #[test]
    fn setext_multiline_with_leading_blank() {
        // §12.7 variant: `\nAlpha\nBeta\n=====` -> preamble row (1-1) + heading row.
        let rows = toc_of("\nAlpha\nBeta\n=====\n");
        let s = rows_summary(&rows);
        assert_eq!(s[0], (1, 0, 1, 1, "preamble")); // line 1 is a blank line
        assert_eq!(s[1], (2, 1, 4, 0, "Alpha Beta"));
    }

    #[test]
    fn empty_atx_heading_has_empty_title() {
        let rows = toc_of("#\nbody\n");
        assert_eq!(rows[0].title, "");
        assert_eq!(rows[0].level, 1);
    }

    #[test]
    fn trailing_atx_hashes_stripped() {
        // `## Foo ##` -> title `Foo`.
        let rows = toc_of("## Foo ##\n");
        assert_eq!(rows[0].title, "Foo");
    }

    #[test]
    fn inline_markup_in_title() {
        // Spaces between inline elements are Text events and are kept; tag
        // boundaries contribute nothing. Link text kept, URL dropped.
        let rows = toc_of("## a *b* `c` ~~d~~ [e](http://x)\n");
        assert_eq!(rows[0].title, "a b c d e");
    }

    #[test]
    fn multiple_h1s_are_separate_subtrees() {
        let rows = toc_of("# A\nbody a\n# B\nbody b\n");
        // A: line 1, ends before B (line 3). B: line 3, ends at N=4.
        assert_eq!(rows[0].end, 2);
        assert_eq!(rows[1].end, 4);
    }

    #[test]
    fn html_h2_is_not_a_heading() {
        let rows = toc_of("<h2>Not a heading</h2>\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "preamble");
    }

    #[test]
    fn no_headings_is_one_preamble_row() {
        let rows = toc_of("just text\nmore text\n");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0],
            Row {
                line: 1,
                level: 0,
                end: 2,
                chars: 20,
                title: "preamble".into(),
            }
        );
    }

    #[test]
    fn empty_file_has_no_rows() {
        assert!(toc_of("").is_empty());
    }
}
