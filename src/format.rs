//! Markdown TOC emitter (spec §7).
//!
//! Columns are always `line, level, end, chars, title`. Integers are plain
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
}
