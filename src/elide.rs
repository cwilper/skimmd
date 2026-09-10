//! Data-URL image elision (spec §2).
//!
//! [`elide_data_urls`] rewrites data-URL *image* embeds to a short marker so
//! readers (human or LLM) get a section's text without swallowing base64
//! payloads:
//!
//! * `![alt](data:image/png;base64,…)` → `<!-- ![alt](data:…) -->` (markdown
//!   form in normal content; elided without the comment wrap when already
//!   inside a comment or code span)
//! * `src="data:…"` / `src='data:…'` attribute → value replaced with
//!   `data:…` (in practice `<img>`; never comment-wrapped)
//!
//! Guarantees (spec §2.2): line-preserving (one input line → one output
//! line), idempotent, and byte-identical on input with no case-insensitive
//! `data:` substring.
//!
//! Never touched: data URLs in fenced code blocks, in plain (non-image)
//! links, in reference definitions, bare `data:` tokens in prose, and
//! non-data URL targets.
//!
//! `ponytail:` ceilings (spec §9): top-level fences only (blockquoted /
//! indented / list-prefixed fences not tracked); per-line backtick parity for
//! code spans; first-`)` termination of markdown dests; `src` matching is
//! attribute-level, so a `src` inside a quoted attribute value of another
//! attribute may be misidentified.

/// Fixed replacement for an elided payload (spec §2.2). `…` is U+2026.
const MARKER: &str = "data:…";

struct State {
    /// Open fence: (marker char, run length). `None` = outside a fence.
    in_fence: Option<(u8, usize)>,
    /// Inside an HTML comment (may span lines).
    in_comment: bool,
}

/// Rewrite data-URL image embeds in `text` to [`MARKER`] (spec §2).
#[must_use]
pub fn elide_data_urls(text: &str) -> String {
    // No case-insensitive `data:` substring ⇒ no elision is possible.
    if find_ci(text, b"data:").is_none() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut st = State {
        in_fence: None,
        in_comment: false,
    };
    for line in text.split_inclusive('\n') {
        out.push_str(&process_line(line, &mut st));
    }
    out
}

/// Process one line (including its trailing `\n`, if any).
fn process_line(line: &str, st: &mut State) -> String {
    // Comment takes precedence over fence bookkeeping: a `<!--` line is
    // comment content, never a fence delimiter (CommonMark HTML block).
    if st.in_comment {
        return scan_line(line, st);
    }
    if st.in_fence.is_some() {
        // Interior fence line: a matching closer ends the fence; either way
        // the line passes through untouched (quoted code stays verbatim).
        if let Some((c, n, e)) = fence_marker(line) {
            if let Some((c0, n0)) = st.in_fence {
                if c == c0 && n >= n0 && line[e..].trim().is_empty() {
                    st.in_fence = None;
                }
            }
        }
        return line.to_string();
    }
    if let Some((c, n, _)) = fence_marker(line) {
        st.in_fence = Some((c, n));
        return line.to_string();
    }
    scan_line(line, st)
}

/// Leading fence marker: the first non-whitespace run of `` ` `` or `~`,
/// length ≥ 3. Returns (char, run length, byte offset after the run).
/// `ponytail:` top-level fences only (no blockquote/list/indent tracking).
fn fence_marker(line: &str) -> Option<(u8, usize, usize)> {
    let b = line.as_bytes();
    let mut i = 0;
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    if i >= b.len() {
        return None;
    }
    let c = b[i];
    if c != b'`' && c != b'~' {
        return None;
    }
    let start = i;
    while i < b.len() && b[i] == c {
        i += 1;
    }
    let n = i - start;
    (n >= 3).then_some((c, n, i))
}

enum Marker {
    MdImage,
    SrcAttr,
    CommentOpen,
    CommentClose,
}

/// Leftmost marker at/after byte `i`: `![`, a `src` attribute candidate,
/// `<!--`, `-->`.
fn next_marker(line: &str, i: usize) -> Option<(usize, Marker)> {
    let b = line.as_bytes();
    let mut best: Option<(usize, Marker)> = None;
    let consider = |pos: usize, m: Marker, best: &mut Option<(usize, Marker)>| {
        if best.as_ref().is_none_or(|b| pos < b.0) {
            *best = Some((pos, m));
        }
    };
    if let Some(p) = line[i..].find("![") {
        consider(i + p, Marker::MdImage, &mut best);
    }
    // `src` attribute candidate: the first boundary-valid occurrence (a `src`
    // embedded in a longer word, e.g. `datasrc`, is skipped, and the search
    // continues past it).
    let mut off = i;
    while off < b.len() {
        match find_ci(&line[off..], b"src") {
            None => break,
            Some(p) => {
                let q = off + p;
                if q == 0 || b[q - 1] == b'<' || b[q - 1].is_ascii_whitespace() {
                    consider(q, Marker::SrcAttr, &mut best);
                    break;
                }
                off = q + 3;
            }
        }
    }
    if let Some(p) = line[i..].find("<!--") {
        consider(i + p, Marker::CommentOpen, &mut best);
    }
    if let Some(p) = line[i..].find("-->") {
        consider(i + p, Marker::CommentClose, &mut best);
    }
    best
}

/// Scan a non-fence line for image embeds, appending the result to `out`.
fn scan_line(line: &str, st: &mut State) -> String {
    let b = line.as_bytes();
    let mut out = String::with_capacity(line.len() + 32);
    let mut i = 0;
    let mut in_comment = st.in_comment;
    while i < b.len() {
        let Some((pos, kind)) = next_marker(line, i) else {
            out.push_str(&line[i..]);
            break;
        };
        out.push_str(&line[i..pos]);
        match kind {
            Marker::CommentOpen => {
                out.push_str("<!--");
                in_comment = true;
                i = pos + 4;
            }
            Marker::CommentClose => {
                out.push_str("-->");
                in_comment = false;
                i = pos + 3;
            }
            Marker::MdImage => {
                i = pos + md_image(line, pos, in_comment, &mut out);
            }
            Marker::SrcAttr => {
                i = pos + src_attr(line, pos, &mut out);
            }
        }
    }
    st.in_comment = in_comment;
    out
}

/// Process a markdown image at `pos` (the `!`), appending to `out`. Returns
/// input bytes consumed — always ≥ 1, so the caller always makes progress.
fn md_image(line: &str, pos: usize, in_comment: bool, out: &mut String) -> usize {
    let b = line.as_bytes();
    let alt_close = match line[pos + 2..].find(']') {
        Some(o) => pos + 2 + o,
        None => {
            out.push('!');
            return 1;
        }
    };
    let open = alt_close + 1;
    if open >= b.len() || b[open] != b'(' {
        out.push('!');
        return 1;
    }
    let mut j = open + 1;
    while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
        j += 1;
    }
    let dest_start = j;
    while j < b.len() && !matches!(b[j], b')' | b' ' | b'\t' | b'\n') {
        j += 1;
    }
    // `ponytail:` first-`)` termination — an unbalanced `)` inside a
    // non-b64 data URI yields a partial elision (spec §9).
    let close = match line[dest_start..].find(')') {
        Some(o) => dest_start + o,
        None => {
            out.push('!');
            return 1;
        }
    };
    let dest = &line[dest_start..j];
    if !starts_with_ci(dest, "data:") {
        out.push_str(&line[pos..close + 1]);
        return close + 1 - pos;
    }
    let alt = &line[pos + 2..alt_close];
    let tail = &line[j..close]; // whitespace + optional link title, verbatim
    let token = format!("![{alt}]({MARKER}{tail})");
    // `ponytail:` per-line backtick parity for code-span detection (spec §9).
    let in_code_span = line[..pos].bytes().filter(|&c| c == b'`').count() % 2 == 1;
    if !in_comment && !in_code_span {
        out.push_str("<!-- ");
        out.push_str(&token);
        out.push_str(" -->");
    } else {
        out.push_str(&token);
    }
    close + 1 - pos
}

/// Process a `src`-attribute candidate at `pos` (the `s`), appending to
/// `out`. Returns input bytes consumed — always ≥ 1.
fn src_attr(line: &str, pos: usize, out: &mut String) -> usize {
    let b = line.as_bytes();
    let mut j = pos + 3;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= b.len() || b[j] != b'=' {
        out.push('s');
        return 1;
    }
    j += 1;
    while j < b.len() && b[j].is_ascii_whitespace() {
        j += 1;
    }
    if j >= b.len() || !(b[j] == b'"' || b[j] == b'\'') {
        out.push('s');
        return 1;
    }
    let quote = b[j] as char;
    let value_start = j + 1;
    let value_end = match line[value_start..].find(quote) {
        Some(o) => value_start + o,
        None => {
            out.push('s');
            return 1;
        }
    };
    let value = &line[value_start..value_end];
    if !starts_with_ci(value, "data:") {
        out.push_str(&line[pos..value_end + 1]);
        return value_end + 1 - pos;
    }
    out.push_str(&line[pos..value_start]);
    out.push_str(MARKER);
    out.push(quote); // closing quote
    value_end + 1 - pos
}

/// Case-insensitive ASCII search for `needle` (lowercase bytes) in `hay`.
fn find_ci(hay: &str, needle: &[u8]) -> Option<usize> {
    let hb = hay.as_bytes();
    if hb.len() < needle.len() {
        return None;
    }
    (0..=hb.len() - needle.len())
        .find(|&i| (0..needle.len()).all(|j| hb[i + j].to_ascii_lowercase() == needle[j]))
}

/// Case-insensitive prefix test for ASCII `needle` in `hay`.
fn starts_with_ci(hay: &str, needle: &str) -> bool {
    hay.len() >= needle.len() && hay[..needle.len()].eq_ignore_ascii_case(needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elide(s: &str) -> String {
        elide_data_urls(s)
    }

    #[test]
    fn markdown_b64_wrapped() {
        assert_eq!(
            elide("![alt](data:image/png;base64,AAA)"),
            "<!-- ![alt](data:…) -->"
        );
    }

    #[test]
    fn markdown_empty_alt() {
        assert_eq!(
            elide("![](data:image/gif;base64,AA)"),
            "<!-- ![](data:…) -->"
        );
    }

    #[test]
    fn markdown_keeps_title() {
        assert_eq!(
            elide("![a](data:image/png;base64,AA \"cap\")"),
            "<!-- ![a](data:… \"cap\") -->"
        );
    }

    #[test]
    fn markdown_data_uri_non_b64() {
        assert_eq!(
            elide("![a](data:image/svg+xml;utf8,<svg></svg>)"),
            "<!-- ![a](data:…) -->"
        );
    }

    #[test]
    fn url_image_untouched() {
        let s = "![a](https://x.example/i.png)";
        assert_eq!(elide(s), s);
    }

    #[test]
    fn plain_data_link_untouched() {
        let s = "[a](data:text/plain;base64,AA)";
        assert_eq!(elide(s), s);
    }

    #[test]
    fn prose_data_untouched() {
        let s = "see data:foo for details";
        assert_eq!(elide(s), s);
    }

    #[test]
    fn reference_def_untouched() {
        let s = "[a]: data:image/png;base64,AA";
        assert_eq!(elide(s), s);
    }

    #[test]
    fn src_double_quoted() {
        assert_eq!(
            elide("<img src=\"data:image/svg+xml;utf8,<svg></svg>\" alt=\"x\">"),
            "<img src=\"data:…\" alt=\"x\">"
        );
    }

    #[test]
    fn src_single_quoted() {
        assert_eq!(
            elide("<img src='data:image/png;base64,AA'>"),
            "<img src='data:…'>"
        );
    }

    #[test]
    fn src_not_first_attribute() {
        assert_eq!(
            elide("<img alt=\"x\" src=\"data:image/png;base64,AA\">"),
            "<img alt=\"x\" src=\"data:…\">"
        );
    }

    #[test]
    fn src_url_value_untouched() {
        let s = "<img src=\"https://x/i.png\" alt=\"x\">";
        assert_eq!(elide(s), s);
    }

    #[test]
    fn src_uppercase_tag_and_attr() {
        assert_eq!(
            elide("<IMG SRC=\"data:image/png;base64,AA\">"),
            "<IMG SRC=\"data:…\">"
        );
    }

    #[test]
    fn src_uppercase_data_prefix() {
        assert_eq!(
            elide("![a](DATA:image/png;base64,AA)"),
            "<!-- ![a](data:…) -->"
        );
    }

    #[test]
    fn src_other_tag_elided() {
        assert_eq!(
            elide("<script src=\"data:text/javascript;base64,AA\"></script>"),
            "<script src=\"data:…\"></script>"
        );
    }

    #[test]
    fn src_not_an_attribute_boundary() {
        // `src` embedded in a longer word is not an attribute.
        let s = "<a datasrc=\"data:image/png;base64,AA\">x</a>";
        assert_eq!(elide(s), s);
    }

    #[test]
    fn inside_single_line_comment() {
        let s = "<!-- see ![a](data:image/png;base64,AA) -->";
        assert_eq!(elide(s), "<!-- see ![a](data:…) -->");
    }

    #[test]
    fn inside_multiline_comment() {
        let s = "<!-- note\n![a](data:image/png;base64,AA)\nend -->\n";
        assert_eq!(elide(s), "<!-- note\n![a](data:…)\nend -->\n");
    }

    #[test]
    fn after_comment_close_same_line_wraps() {
        let s = "<!-- c --> ![a](data:image/png;base64,AA)\n";
        assert_eq!(elide(s), "<!-- c --> <!-- ![a](data:…) -->\n");
    }

    #[test]
    fn fence_ticks_untouched() {
        let s = "before\n```\n![a](data:image/png;base64,AA)\n```\nafter ![b](data:image/png;base64,BB)\n";
        assert_eq!(
            elide(s),
            "before\n```\n![a](data:image/png;base64,AA)\n```\nafter <!-- ![b](data:…) -->\n"
        );
    }

    #[test]
    fn fence_tildes_and_info_string() {
        let s = "~~~ svg\n<img src=\"data:image/png;base64,AA\">\n~~~\n![b](data:image/png;base64,BB)\n";
        assert_eq!(
            elide(s),
            "~~~ svg\n<img src=\"data:image/png;base64,AA\">\n~~~\n<!-- ![b](data:…) -->\n"
        );
    }

    #[test]
    fn fence_interior_marker_line_untouched() {
        // A `~~~` line inside a ``` fence is content, not a closer.
        let s = "```\n~~~\n![a](data:image/png;base64,AA)\n```\n";
        assert_eq!(elide(s), s);
    }

    #[test]
    fn inline_code_span_elided_no_wrap() {
        let s = "text `![a](data:image/png;base64,AA)` more\n";
        assert_eq!(elide(s), "text `![a](data:…)` more\n");
    }

    #[test]
    fn idempotent() {
        let s = "x ![a](data:image/png;base64,AA) <img src='data:image/b;x'>\n";
        let once = elide(s);
        assert_eq!(elide(&once), once);
    }

    #[test]
    fn clean_input_identical() {
        let s = "# t\njust text, no embeds\n";
        assert_eq!(elide(s), s);
    }

    #[test]
    fn empty_input() {
        assert_eq!(elide(""), "");
    }

    #[test]
    fn line_count_preserved() {
        let s =
            "a\n![x](data:image/png;base64,AA)\n<!-- c\n![y](data:image/png;base64,BB)\nd -->\n";
        assert_eq!(elide(s).lines().count(), s.lines().count());
    }

    #[test]
    fn multiple_images_one_line() {
        let s = "![a](data:image/png;base64,AA) mid ![b](data:image/png;base64,BB)\n";
        assert_eq!(
            elide(s),
            "<!-- ![a](data:…) --> mid <!-- ![b](data:…) -->\n"
        );
    }
}
