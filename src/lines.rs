//! The 1-based physical line model, plus file reading (BOM strip + UTF-8 validation).
//!
//! Lines are delimited by `\n` only. A line **includes its own `\n`** if present.
//! Line numbers are 1-based and map 1:1 to `grep -n`, `sed -n 'a,bp'`, and editors.
//!
//! * A lone `\r` is an ordinary character (classic-Mac line endings are unsupported).
//! * In CRLF files the `\r` stays on its line, is output verbatim, and counts in `chars`.

use std::io::Read;
use std::path::Path;

/// A line map over an in-memory, BOM-stripped, valid-UTF-8 `text`.
#[derive(Debug, Clone)]
pub struct LineMap {
    text: String,
    /// `starts[i]` is the byte offset of line `i + 1` (1-based). Length == `n`.
    starts: Vec<usize>,
    /// Number of lines. `0` for an empty `text`.
    n: usize,
}

impl LineMap {
    /// Build the line-start table from decoded (BOM-stripped) text.
    pub fn new(text: String) -> Self {
        let mut starts = vec![0usize];
        for (j, b) in text.bytes().enumerate() {
            if b == b'\n' && j + 1 < text.len() {
                starts.push(j + 1);
            }
        }
        let n = if text.is_empty() { 0 } else { starts.len() };
        LineMap { text, starts, n }
    }

    /// Number of lines `N`.
    #[must_use]
    pub fn n(&self) -> usize {
        self.n
    }

    /// The full decoded text (BOM already stripped).
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 1-based line number containing byte offset `o`.
    ///
    /// Intended for valid offsets (`n > 0`, `o < text.len()`); out-of-range `o`
    /// simply clamps to the last line rather than panicking.
    #[must_use]
    pub fn line_of(&self, o: usize) -> usize {
        self.starts.partition_point(|&s| s <= o)
    }

    /// Byte span for lines `a..=b` (1-based, inclusive). Returns `""` when `a > b`
    /// or `a` is out of range.
    #[must_use]
    pub fn span(&self, a: usize, b: usize) -> &str {
        if a < 1 || a > b || a > self.n {
            return "";
        }
        let start = self.starts[a - 1];
        let end = if b < self.n {
            self.starts[b]
        } else {
            self.text.len()
        };
        &self.text[start..end]
    }

    /// Character (not byte) count of `span(a, b)`.
    #[must_use]
    pub fn chars(&self, a: usize, b: usize) -> usize {
        self.span(a, b).chars().count()
    }
}

/// A file could not be turned into decoded text.
#[derive(Debug)]
pub enum LoadErr {
    /// `std::fs::read` failed (missing file, is a directory, permission, …).
    Io(std::io::Error),
    /// The bytes (post-BOM) were not valid UTF-8.
    NotUtf8,
}

/// Read a file, strip a leading UTF-8 BOM, and validate UTF-8.
///
/// Returns the decoded, BOM-stripped text (the spec's `text`). The BOM is never
/// counted, output, or passed onward. No lossy decoding.
pub fn load_text(path: &Path) -> Result<String, LoadErr> {
    decode(&std::fs::read(path).map_err(LoadErr::Io)?)
}

/// Read all of stdin, strip a leading UTF-8 BOM, and validate UTF-8.
pub fn load_stdin() -> Result<String, LoadErr> {
    let mut bytes = Vec::new();
    std::io::stdin().read_to_end(&mut bytes).map_err(LoadErr::Io)?;
    decode(&bytes)
}

/// Strip a leading UTF-8 BOM and decode as strict UTF-8.
fn decode(bytes: &[u8]) -> Result<String, LoadErr> {
    let bytes = bytes
        .strip_prefix([0xEF, 0xBB, 0xBF].as_slice())
        .unwrap_or(bytes);
    String::from_utf8(bytes.to_vec()).map_err(|_| LoadErr::NotUtf8)
}

/// Reason text (no `skimmd:` prefix, no path) for a `std::io` read failure.
///
/// `NotFound` and `IsADirectory` use the spec's exact phrases; any other kind
/// falls back to the OS error text.
#[must_use]
pub fn io_reason(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => "No such file or directory".into(),
        std::io::ErrorKind::IsADirectory => "is a directory".into(),
        _ => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lm(s: &str) -> LineMap {
        LineMap::new(s.to_string())
    }

    #[test]
    fn empty() {
        let l = lm("");
        assert_eq!(l.n(), 0);
        assert_eq!(l.span(1, 1), "");
    }

    #[test]
    fn lone_newline() {
        let l = lm("\n");
        assert_eq!(l.n(), 1);
        assert_eq!(l.span(1, 1), "\n");
    }

    #[test]
    fn single_no_newline() {
        let l = lm("a");
        assert_eq!(l.n(), 1);
        assert_eq!(l.span(1, 1), "a");
        assert_eq!(l.line_of(0), 1);
    }

    #[test]
    fn single_with_newline() {
        let l = lm("a\n");
        assert_eq!(l.n(), 1);
        assert_eq!(l.span(1, 1), "a\n");
    }

    #[test]
    fn two_lines() {
        let l = lm("a\nb");
        assert_eq!(l.n(), 2);
        assert_eq!(l.span(1, 1), "a\n");
        assert_eq!(l.span(2, 2), "b");
        assert_eq!(l.span(1, 2), "a\nb");
        // offsets: 0->'a', 1->'\n' (line 1), 2->'b' (line 2)
        assert_eq!(l.line_of(0), 1);
        assert_eq!(l.line_of(1), 1);
        assert_eq!(l.line_of(2), 2);
        assert_eq!(l.chars(1, 2), 3);
    }

    #[test]
    fn trailing_blank_line() {
        let l = lm("a\n\n");
        assert_eq!(l.n(), 2);
        assert_eq!(l.span(1, 1), "a\n");
        assert_eq!(l.span(2, 2), "\n");
        assert_eq!(l.span(1, 2), "a\n\n");
    }

    #[test]
    fn crlf_keeps_cr_on_line() {
        let l = lm("a\r\nb\r\n");
        assert_eq!(l.n(), 2);
        assert_eq!(l.span(1, 1), "a\r\n");
        assert_eq!(l.span(2, 2), "b\r\n");
        assert_eq!(l.chars(1, 1), 3); // a, \r, \n
        assert_eq!(l.line_of(2), 1);
        assert_eq!(l.line_of(3), 2);
    }

    #[test]
    fn lone_cr_is_not_a_line_terminator() {
        let l = lm("a\rb");
        assert_eq!(l.n(), 1);
        assert_eq!(l.span(1, 1), "a\rb");
        assert_eq!(l.chars(1, 1), 3);
    }

    #[test]
    fn multibyte_chars_counted_not_bytes() {
        let l = lm("héllo\n"); // é is 2 bytes, 1 char
        assert_eq!(l.n(), 1);
        assert_eq!(l.chars(1, 1), 6); // h,é,l,l,o,\n
        assert_eq!(l.span(1, 1).len(), 7); // bytes
    }

    #[test]
    fn span_invalid_ranges_are_empty() {
        let l = lm("a\nb");
        assert_eq!(l.span(3, 2), ""); // a > b
        assert_eq!(l.span(0, 2), ""); // a < 1
        assert_eq!(l.chars(3, 3), 0);
    }

    #[test]
    fn line_of_clamps_out_of_range() {
        let l = lm("a\nb");
        assert_eq!(l.line_of(100), 2); // beyond end -> last line, no panic
    }

    // --- load_text / BOM / io_reason -----------------------------------------

    #[test]
    fn load_strips_bom() {
        let p = std::env::temp_dir().join("skimmd_test_bom.md");
        std::fs::write(&p, [0xEF, 0xBB, 0xBF, b'#', b't'].as_slice()).unwrap();
        let t = load_text(&p).unwrap();
        assert_eq!(t, "#t"); // BOM gone
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn load_invalid_utf8() {
        let p = std::env::temp_dir().join("skimmd_test_bad.md");
        std::fs::write(&p, [0xFF, 0xFE, 0x00]).unwrap();
        assert!(matches!(load_text(&p), Err(LoadErr::NotUtf8)));
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn io_reason_not_found() {
        let e = std::fs::read("/nonexistent/skimmd_nope.md").unwrap_err();
        assert_eq!(io_reason(&e), "No such file or directory");
    }

    #[test]
    fn io_reason_is_a_directory() {
        let e = std::fs::read(std::path::Path::new("/")).unwrap_err();
        assert_eq!(io_reason(&e), "is a directory");
    }
}
