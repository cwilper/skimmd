//! md / tsv / json TOC emitters (spec §7).
//!
//! Column order in every format is `line, level, end, chars, title`. Integers are
//! plain decimal. Output ends with exactly one `\n`. Header rows are always
//! present, even when there are zero data rows.

use clap::ValueEnum;
use serde::Serialize;

use crate::toc::Row;

/// TOC output format. `--format` (TOC mode only; ignored in range mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum Format {
    #[default]
    Md,
    Tsv,
    Json,
}

/// Render `rows` (with total file length `lines`) in `fmt`.
#[must_use]
pub fn render(rows: &[Row], lines: usize, fmt: Format) -> String {
    match fmt {
        Format::Md => render_md(rows),
        Format::Tsv => render_tsv(rows),
        Format::Json => render_json(rows, lines),
    }
}

/// Compact GitHub-style table (§7.1). Only `|` needs escaping in titles.
fn render_md(rows: &[Row]) -> String {
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

/// Tab-separated (§7.2). No escaping needed (titles have no tabs/newlines).
fn render_tsv(rows: &[Row]) -> String {
    let mut out = String::from("line\tlevel\tend\tchars\ttitle\n");
    for r in rows {
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            r.line, r.level, r.end, r.chars, r.title
        ));
    }
    out
}

/// Compact single-line JSON (§7.3). Key order `lines` then `toc`; per row
/// `line, level, end, chars, title` — preserved by serde's field order.
fn render_json(rows: &[Row], lines: usize) -> String {
    #[derive(Serialize)]
    struct Doc<'a> {
        lines: usize,
        toc: &'a [Row],
    }
    // All fields are integers and strings, so this cannot fail.
    let s = serde_json::to_string(&Doc { lines, toc: rows })
        .expect("serializing integers/strings cannot fail");
    format!("{s}\n")
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
    fn md_empty_has_header_only() {
        assert_eq!(
            render(&[], 0, Format::Md),
            "| line | level | end | chars | title |\n|---|---|---|---|---|\n"
        );
    }

    #[test]
    fn md_escapes_pipe() {
        let rows = vec![row(1, 0, 7, 74, "Usage | Notes")];
        assert_eq!(
            render(&rows, 7, Format::Md),
            "| line | level | end | chars | title |\n|---|---|---|---|---|\n| 1 | 0 | 7 | 74 | Usage \\| Notes |\n"
        );
    }

    #[test]
    fn md_empty_title_renders_blank_cell() {
        let rows = vec![row(1, 1, 1, 0, "")];
        assert_eq!(
            render(&rows, 1, Format::Md),
            "| line | level | end | chars | title |\n|---|---|---|---|---|\n| 1 | 1 | 1 | 0 |  |\n"
        );
    }

    #[test]
    fn tsv_empty_has_header_only() {
        assert_eq!(
            render(&[], 0, Format::Tsv),
            "line\tlevel\tend\tchars\ttitle\n"
        );
    }

    #[test]
    fn tsv_pipe_is_raw() {
        let rows = vec![row(1, 0, 7, 74, "Usage | Notes")];
        assert_eq!(
            render(&rows, 7, Format::Tsv),
            "line\tlevel\tend\tchars\ttitle\n1\t0\t7\t74\tUsage | Notes\n"
        );
    }

    #[test]
    fn json_key_order_and_compactness() {
        let rows = vec![
            row(1, 0, 7, 74, "preamble"),
            row(8, 1, 30, 17, "Usage | Notes"),
        ];
        assert_eq!(
            render(&rows, 30, Format::Json),
            "{\"lines\":30,\"toc\":[{\"line\":1,\"level\":0,\"end\":7,\"chars\":74,\"title\":\"preamble\"},{\"line\":8,\"level\":1,\"end\":30,\"chars\":17,\"title\":\"Usage | Notes\"}]}\n"
        );
    }

    #[test]
    fn json_empty_file() {
        assert_eq!(render(&[], 0, Format::Json), "{\"lines\":0,\"toc\":[]}\n");
    }
}
