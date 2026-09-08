//! Markdown TOC emitter (spec §7).
//!
//! Columns are always `line, level, end, chars, title` — and with the `-f`
//! filter, `render_filtered` appends a `matches` count after `title`. Integers are plain
//! decimal. Output ends with exactly one `\n`. The header row is always present,
//! even when there are zero data rows.

use crate::toc::Row;

/// Render `rows` as a compact GitHub-style Markdown table. Only `|` needs
/// escaping in titles.
#[must_use]
pub fn render(rows: &[Row]) -> String {
    let mut out = String::from("| line | level | end | chars | title |\n|---|---|---|---|---|\n");
    for r in rows {
        let title = r.title.replace('|', "\\|");
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            r.line, r.level, r.end, r.chars, title
        ));
    }
    out
}

/// Render *filtered* rows — each a `(Row, matches)` pair — with the extra
/// `matches` column after `title`, so the first five columns stay
/// positionally identical to the unfiltered [`render`] output. Emits only when
/// `-f` is given; the unfiltered 5-column table stays in [`render`].
#[must_use]
pub fn render_filtered(rows: &[(Row, usize)]) -> String {
    let mut out = String::from(
        "| line | level | end | chars | title | matches |\n|---|---|---|---|---|---|\n",
    );
    for (r, m) in rows {
        let title = r.title.replace('|', "\\|");
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            r.line, r.level, r.end, r.chars, title, m
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(line: usize, level: u8, end: usize, chars: usize, title: &str) -> Row {
        Row {
            line,
            level,
            end,
            chars,
            title: title.into(),
        }
    }

    #[test]
    fn empty_has_header_only() {
        assert_eq!(
            render(&[]),
            "| line | level | end | chars | title |\n|---|---|---|---|---|\n"
        );
    }

    #[test]
    fn escapes_pipe() {
        let rows = vec![row(1, 0, 7, 74, "Usage | Notes")];
        assert_eq!(
            render(&rows),
            "| line | level | end | chars | title |\n|---|---|---|---|---|\n| 1 | 0 | 7 | 74 | Usage \\| Notes |\n"
        );
    }

    #[test]
    fn empty_title_renders_blank_cell() {
        let rows = vec![row(1, 1, 1, 0, "")];
        assert_eq!(
            render(&rows),
            "| line | level | end | chars | title |\n|---|---|---|---|---|\n| 1 | 1 | 1 | 0 |  |\n"
        );
    }

    #[test]
    fn filtered_has_matches_column_after_title() {
        let rows = vec![(row(1, 1, 1, 0, "X"), 5)];
        assert_eq!(
            render_filtered(&rows),
            "| line | level | end | chars | title | matches |\n|---|---|---|---|---|---|\n| 1 | 1 | 1 | 0 | X | 5 |\n"
        );
    }

    #[test]
    fn filtered_escapes_pipe_in_title() {
        let rows = vec![(row(2, 2, 5, 9, "A | B"), 0)];
        assert_eq!(
            render_filtered(&rows),
            "| line | level | end | chars | title | matches |\n|---|---|---|---|---|---|\n| 2 | 2 | 5 | 9 | A \\| B | 0 |\n"
        );
    }

    #[test]
    fn filtered_empty_has_header_only() {
        assert_eq!(
            render_filtered(&[]),
            "| line | level | end | chars | title | matches |\n|---|---|---|---|---|---|\n"
        );
    }
}
