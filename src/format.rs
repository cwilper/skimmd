//! Markdown TOC emitter (spec §7).
//!
//! Columns are always `line, level, end, chars, elided, title` — and with
//! the `-f` filter, `render_filtered` appends a `matches` count after
//! `title`. `chars` counts the section body after the default data-URL
//! elision; `elided` how many chars that elision removed. Integers are plain
//! decimal.
//! Output ends with exactly one `\n`. The header row is always present,
//! even when there are zero data rows.

use crate::toc::Row;

/// The six shared cells of a TOC row (through `title`); both table layouts
/// build on this so the column order can't drift. Only `|` needs escaping.
fn row_cells(r: &Row) -> String {
    format!(
        "{} | {} | {} | {} | {} | {}",
        r.line,
        r.level,
        r.end,
        r.chars,
        r.elided,
        r.title.replace('|', "\\|")
    )
}

/// Render `rows` as a compact GitHub-style Markdown table.
#[must_use]
pub fn render(rows: &[Row]) -> String {
    let mut out = String::from(
        "| line | level | end | chars | elided | title |\n|---|---|---|---|---|---|\n",
    );
    for r in rows {
        out.push_str(&format!("| {} |\n", row_cells(r)));
    }
    out
}

/// Render *filtered* rows — each a `(Row, matches)` pair — with the extra
/// `matches` column after `title`, so the first six columns stay
/// positionally identical to the unfiltered [`render`] output. Emits only when
/// `-f` is given; the unfiltered 6-column table stays in [`render`].
#[must_use]
pub fn render_filtered(rows: &[(Row, usize)]) -> String {
    let mut out = String::from(
        "| line | level | end | chars | elided | title | matches |\n|---|---|---|---|---|---|---|\n",
    );
    for (r, m) in rows {
        out.push_str(&format!("| {} | {} |\n", row_cells(r), m));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(line: usize, level: u8, end: usize, chars: usize, elided: usize, title: &str) -> Row {
        Row {
            line,
            level,
            end,
            chars,
            elided,
            title: title.into(),
        }
    }

    #[test]
    fn empty_has_header_only() {
        assert_eq!(
            render(&[]),
            "| line | level | end | chars | elided | title |\n|---|---|---|---|---|---|\n"
        );
    }

    #[test]
    fn escapes_pipe() {
        let rows = vec![row(1, 0, 7, 74, 0, "Usage | Notes")];
        assert_eq!(
            render(&rows),
            "| line | level | end | chars | elided | title |\n|---|---|---|---|---|---|\n| 1 | 0 | 7 | 74 | 0 | Usage \\| Notes |\n"
        );
    }

    #[test]
    fn empty_title_renders_blank_cell() {
        let rows = vec![row(1, 1, 1, 0, 0, "")];
        assert_eq!(
            render(&rows),
            "| line | level | end | chars | elided | title |\n|---|---|---|---|---|---|\n| 1 | 1 | 1 | 0 | 0 |  |\n"
        );
    }

    #[test]
    fn chars_and_elided_both_rendered() {
        let rows = vec![row(1, 1, 9, 10, 500, "X")];
        assert_eq!(
            render(&rows),
            "| line | level | end | chars | elided | title |\n|---|---|---|---|---|---|\n| 1 | 1 | 9 | 10 | 500 | X |\n"
        );
    }

    #[test]
    fn filtered_has_matches_column_after_title() {
        let rows = vec![(row(1, 1, 1, 0, 0, "X"), 5)];
        assert_eq!(
            render_filtered(&rows),
            "| line | level | end | chars | elided | title | matches |\n|---|---|---|---|---|---|---|\n| 1 | 1 | 1 | 0 | 0 | X | 5 |\n"
        );
    }

    #[test]
    fn filtered_empty_has_header_only() {
        assert_eq!(
            render_filtered(&[]),
            "| line | level | end | chars | elided | title | matches |\n|---|---|---|---|---|---|---|\n"
        );
    }
}
