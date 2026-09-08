//! Heading extraction and TOC row computation.
//!
//! See spec §5 (heading extraction) and §6 (TOC row computation). Every row has
//! exactly five fields in this order: `line`, `level`, `end`, `chars`, `title`.
//!
//! Two distinct right-hand boundaries are computed per heading:
//! * `chars` stops at the **next heading of any level** (the heading's own body).
//! * `end` stops before the **next heading of equal or shallower level** (subtree).

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::lines::LineMap;

/// One TOC row. Field order is load-bearing (matches the Markdown emitter).
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Keep rows whose section text matches `needle`, case- and whitespace-insensitively.
/// A row's block is its heading line(s) plus its own body: `line` up to the line
/// before the next row's heading, or EOF (subsections excluded). The needle and the
/// section text are lowercased and whitespace-normalized (every run of whitespace,
/// incl. newlines, collapses to one space) before a plain `str::contains` — no regex.
#[must_use]
pub fn filter_rows(lm: &LineMap, rows: &[Row], needle: &str) -> Vec<Row> {
    let needle = normalize_ws(&needle.to_lowercase());
    let n = lm.n();
    rows.iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let block_end = rows.get(i + 1).map_or(n, |next| next.line - 1);
            let hay = normalize_ws(&lm.span(r.line, block_end).to_lowercase());
            hay.contains(&needle).then(|| r.clone())
        })
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
                        title: normalize_ws(&buf),
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

/// Collapse every run of whitespace (spaces, tabs, newlines, `\r`) to a single space
/// and trim the ends. Used for heading titles (§5.2 step 5) and for the `-f` filter
/// (normalizing the keyword and section text before matching).
#[must_use]
fn normalize_ws(s: &str) -> String {
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

    // --- filter_rows (the -f/--filter filter) --------------------------------

    #[test]
    fn filter_matches_title_and_body_case_insensitively() {
        let lm = LineMap::new("# Alpha\nalpha body\n## Beta\nonly beta\n".to_string());
        let rows = build_toc(&lm);
        // "beta" is in the Beta heading and body; "ALPHA" (case-insens.) in Alpha.
        assert_eq!(filter_rows(&lm, &rows, "beta")[0].title, "Beta");
        assert_eq!(filter_rows(&lm, &rows, "ALPHA").len(), 1);
        assert_eq!(filter_rows(&lm, &rows, "ALPHA")[0].title, "Alpha");
        // body-only match: "alpha" is in Alpha's body, not just its heading.
        assert_eq!(filter_rows(&lm, &rows, "body")[0].title, "Alpha");
        // no match -> empty, no panic.
        assert!(filter_rows(&lm, &rows, "zzz").is_empty());
    }

    #[test]
    fn filter_excludes_subsections() {
        // "deep" is only in the H3's body; the H2's own body must not match it,
        // but the H3 row must.
        let lm = LineMap::new("## Parent\nshallow\n### Child\ndeep text\n".to_string());
        let rows = build_toc(&lm);
        let got = filter_rows(&lm, &rows, "deep");
        assert_eq!(got.len(), 1, "only the leaf section matches: {got:?}");
        assert_eq!(got[0].title, "Child");
    }

    #[test]
    fn filter_normalizes_whitespace_including_newlines() {
        // "foo" ends line 2, "bar" starts line 3: the keyword may span the break.
        // Runs of any whitespace (space/tab/newline) collapse on both sides.
        let lm = LineMap::new("## Sec\nends with foo\nbar starts here\n".to_string());
        let rows = build_toc(&lm);
        assert_eq!(filter_rows(&lm, &rows, "foo bar").len(), 1, "single space");
        assert_eq!(filter_rows(&lm, &rows, "foo  bar").len(), 1, "double space");
        assert_eq!(filter_rows(&lm, &rows, "foo\tbar").len(), 1, "tab");
        assert_eq!(
            filter_rows(&lm, &rows, "foo\nbar").len(),
            1,
            "newline in keyword"
        );
        assert!(
            filter_rows(&lm, &rows, "foo baz").is_empty(),
            "no such phrase"
        );
    }

    // --- §12.8 structural invariants (private access, no public helper needed) -

    /// Edge-case corpus beyond the golden fixture (CRLF, setext, multi-H1, empty…).
    fn corpus() -> &'static [&'static str] {
        &[
            "",
            "# H\n",
            "plain\n",
            "---\ntitle: x\n---\n\n# Real\n",
            "Alpha\nBeta\n=====\n",
            "a\r\nb\r\nc\r\n",
            "# A\n# B\n",
            "> ## Q\n- ## L\n",
            "## x {#id}\nbody\n",
            "#\n",
            "no trailing newline",
            "# T\n\nbody\n",
        ]
    }

    /// §12.8 accounting invariant: preamble + each heading's own lines + each
    /// heading row's body chars must tile the whole file.
    #[test]
    fn accounting_invariant_holds() {
        for (i, text) in corpus().iter().enumerate() {
            let lm = LineMap::new(text.to_string());
            let headings = extract_headings(lm.text(), &lm);
            let rows = build_toc(&lm);
            let n = lm.n();
            let p = headings.first().map_or(n, |h| h.first_line - 1);
            let mut sum = lm.chars(1, p); // 0 when the preamble is suppressed
            for h in &headings {
                sum += lm.chars(h.first_line, h.last_line); // heading's own lines
            }
            for r in rows.iter().filter(|r| r.level > 0) {
                sum += r.chars; // each heading row's body
            }
            assert_eq!(sum, lm.text().chars().count(), "corpus[{i}]: {text:?}");
        }
    }

    #[test]
    fn last_row_end_equals_n() {
        for (i, text) in corpus().iter().enumerate() {
            let lm = LineMap::new(text.to_string());
            let rows = build_toc(&lm);
            let n = lm.n();
            if n == 0 {
                assert!(rows.is_empty(), "corpus[{i}]");
            } else {
                assert_eq!(rows.last().map(|r| r.end), Some(n), "corpus[{i}]: {text:?}");
            }
        }
    }

    #[test]
    fn preamble_row_consistency() {
        for text in ["intro line\n# Head\n", "# Head\n", ""] {
            let lm = LineMap::new(text.to_string());
            let rows = build_toc(&lm);
            if lm.n() == 0 {
                continue;
            }
            let first = &rows[0];
            assert_eq!(first.line, 1);
            if rows.len() > 1 {
                assert_eq!(first.level, 0);
                assert_eq!(first.title, "preamble");
                assert_eq!(first.end, rows[1].line - 1);
            } else {
                assert!(first.level > 0 && first.line == 1, "{text:?}");
            }
        }
    }
}
