//! Range-spec grammar: parse, validate, normalize.
//!
//! Grammar (each positional after `FILE` is a `SPEC`):
//!
//! ```text
//! SPEC  := RANGE ( ',' RANGE )*
//! RANGE := INT '-' INT?
//! INT   := [0-9]+
//! ```
//!
//! * No whitespace inside a `SPEC`; multiple `SPEC` args are pooled (left to right).
//! * Exactly one `-` per `RANGE`; a bare integer is a syntax error.
//! * Leading zeros are permitted and ignored. `N-` means "through the last line".
//! * All line numbers are plain 1-based physical lines; smallest valid start is `1`.
//!
//! Processing is two-phase, so **every spec is parsed fully before any
//! validation** (per spec §8.1): the first syntax error (pool order) wins; only if
//! all specs parse does validation run, and the first validation failure (pool
//! order) wins.

/// A normalized effective range `[start..=end]`, 1-based, inclusive.
/// After `parse`, ranges are sorted by `start` and non-overlapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    pub start: usize,
    pub end: usize,
}

/// A parsed-but-not-yet-validated range, keeping literal tokens for messages.
#[derive(Debug)]
struct RawRange {
    /// The whole range token exactly as written (e.g. `"007-"`).
    raw: String,
    /// The literal start token (e.g. `"007"` or `"31"`).
    raw_start: String,
    /// The literal end token, if present (e.g. `Some("999")`).
    raw_end: Option<String>,
    start: usize,
    end: Option<usize>,
}

/// A range error. The stored text is the message body after the `skimmd: ` prefix.
#[derive(Debug)]
pub enum RangeErr {
    /// `invalid range spec '{0}': expected N-M or N- (comma-separated)`
    Syntax(String),
    /// `range {0}: start must be at least 1`
    StartTooSmall(String),
    /// `range {0}: end is less than start`
    EndBeforeStart(String),
    /// `range {0}: start {1} exceeds file length ({2} lines)`
    StartExceeds(String, String, usize),
    /// `range {0}: end {1} exceeds file length ({2} lines)`
    EndExceeds(String, String, usize),
}

impl RangeErr {
    /// The full message body (without the `skimmd: ` prefix).
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            RangeErr::Syntax(s) => {
                format!("invalid range spec '{s}': expected N-M or N- (comma-separated)")
            }
            RangeErr::StartTooSmall(s) => format!("range {s}: start must be at least 1"),
            RangeErr::EndBeforeStart(s) => format!("range {s}: end is less than start"),
            RangeErr::StartExceeds(s, st, n) => {
                format!("range {s}: start {st} exceeds file length ({n} lines)")
            }
            RangeErr::EndExceeds(s, en, n) => {
                format!("range {s}: end {en} exceeds file length ({n} lines)")
            }
        }
    }
}

/// Parse, validate against a file of `n` lines, and normalize `specs`.
///
/// Returns the merged, ascending, non-overlapping effective ranges. The first
/// failing range (pool order) determines any `Err`.
pub fn parse(specs: &[String], n: usize) -> Result<Vec<Range>, RangeErr> {
    // Phase A: syntax — parse every spec fully. First syntax error wins.
    let mut raw: Vec<RawRange> = Vec::new();
    for spec in specs {
        for tok in spec.split(',') {
            raw.push(parse_range(tok, spec)?);
        }
    }

    // Phase B: validation — pool order. First failure wins.
    for r in &raw {
        validate(r, n)?;
    }

    // Phase C: normalize — resolve `end`, sort by start, merge adjacent/overlap.
    let mut eff: Vec<Range> = raw
        .iter()
        .map(|r| Range {
            start: r.start,
            end: r.end.unwrap_or(n),
        })
        .collect();
    eff.sort_by_key(|r| r.start);
    let mut merged: Vec<Range> = Vec::new();
    for r in eff {
        if let Some(last) = merged.last_mut() {
            if r.start <= last.end + 1 {
                last.end = last.end.max(r.end);
            } else {
                merged.push(r);
            }
        } else {
            merged.push(r);
        }
    }
    Ok(merged)
}

/// Parse a single `RANGE` token. `spec` is the whole positional (for the
/// syntax-error message); on failure the token's structure is the problem.
fn parse_range(tok: &str, spec: &str) -> Result<RawRange, RangeErr> {
    let (left, right) = tok
        .split_once('-')
        .ok_or_else(|| RangeErr::Syntax(spec.to_string()))?;
    // `left` must be present and all digits. `right` may be empty (`N-`) or all digits.
    if left.is_empty()
        || !left.bytes().all(|b: u8| b.is_ascii_digit())
        || !right.bytes().all(|b: u8| b.is_ascii_digit())
    {
        return Err(RangeErr::Syntax(spec.to_string()));
    }
    Ok(RawRange {
        raw: tok.to_string(),
        raw_start: left.to_string(),
        raw_end: (if right.is_empty() {
            None
        } else {
            Some(right.to_string())
        }),
        start: parse_usize(left),
        end: if right.is_empty() {
            None
        } else {
            Some(parse_usize(right))
        },
    })
}

/// Parse an all-digit string, saturating to `usize::MAX` on overflow.
/// (A range number that big can never be valid; the sentinel lets validation
/// emit the correct `exceeds file length` message instead of panicking.)
fn parse_usize(s: &str) -> usize {
    s.parse::<usize>().unwrap_or(usize::MAX)
}

/// Apply the four validation checks, in order, to one parsed range.
fn validate(r: &RawRange, n: usize) -> Result<(), RangeErr> {
    if r.start < 1 {
        return Err(RangeErr::StartTooSmall(r.raw.clone()));
    }
    if let Some(e) = r.end {
        if e < r.start {
            return Err(RangeErr::EndBeforeStart(r.raw.clone()));
        }
    }
    if r.start > n {
        return Err(RangeErr::StartExceeds(
            r.raw.clone(),
            r.raw_start.clone(),
            n,
        ));
    }
    if let Some(e) = r.end {
        if e > n {
            return Err(RangeErr::EndExceeds(
                r.raw.clone(),
                r.raw_end.clone().unwrap(),
                n,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse `specs` against an `n`-line file; assert the normalized ranges.
    fn ok(specs: &[&str], n: usize) -> Vec<Range> {
        let v: Vec<String> = specs.iter().map(|s| s.to_string()).collect();
        parse(&v, n).unwrap()
    }

    /// Assert a parse error with the exact message.
    fn err(specs: &[&str], n: usize, want: &str) {
        let v: Vec<String> = specs.iter().map(|s| s.to_string()).collect();
        let got = parse(&v, n).unwrap_err().message();
        assert_eq!(got, want, "spec={specs:?}");
    }

    const N: usize = 30;

    #[test]
    fn good_forms() {
        assert_eq!(ok(&["1-"], N), vec![Range { start: 1, end: 30 }]);
        assert_eq!(ok(&["1-1"], N), vec![Range { start: 1, end: 1 }]);
        assert_eq!(ok(&["007-010"], N), vec![Range { start: 7, end: 10 }]);
        assert_eq!(
            ok(&["1-5,9-12"], N),
            vec![Range { start: 1, end: 5 }, Range { start: 9, end: 12 }]
        );
        // two spec args == one comma-joined spec
        assert_eq!(
            ok(&["1-5", "9-12"], N),
            vec![Range { start: 1, end: 5 }, Range { start: 9, end: 12 }]
        );
    }

    #[test]
    fn merging() {
        assert_eq!(ok(&["5-10", "3-6"], N), vec![Range { start: 3, end: 10 }]);
        assert_eq!(ok(&["5-10,3-6"], N), vec![Range { start: 3, end: 10 }]);
        // adjacent
        assert_eq!(ok(&["1-2,3-4"], N), vec![Range { start: 1, end: 4 }]);
        // overlap
        assert_eq!(ok(&["1-5,4-8"], N), vec![Range { start: 1, end: 8 }]);
        // already sorted + disjoint stays disjoint
        assert_eq!(ok(&["2-3,1-1,4-4"], N), vec![Range { start: 1, end: 4 }]);
        // `N-` at the end merges with anything touching the tail
        assert_eq!(ok(&["20-25,26-"], N), vec![Range { start: 20, end: 30 }]);
    }

    #[test]
    fn syntax_errors() {
        err(
            &["5"],
            N,
            "invalid range spec '5': expected N-M or N- (comma-separated)",
        );
        err(
            &["1-10,x-y"],
            N,
            "invalid range spec '1-10,x-y': expected N-M or N- (comma-separated)",
        );
        err(
            &[""],
            N,
            "invalid range spec '': expected N-M or N- (comma-separated)",
        );
        err(
            &["-"],
            N,
            "invalid range spec '-': expected N-M or N- (comma-separated)",
        );
        err(
            &["1-2-3"],
            N,
            "invalid range spec '1-2-3': expected N-M or N- (comma-separated)",
        );
        // the reported spec is the whole positional, even when one token is bad
        err(
            &["x-1"],
            N,
            "invalid range spec 'x-1': expected N-M or N- (comma-separated)",
        );
    }

    #[test]
    fn validation_errors() {
        err(&["0-"], N, "range 0-: start must be at least 1");
        err(&["0-0"], N, "range 0-0: start must be at least 1");
        err(&["10-5"], N, "range 10-5: end is less than start");
        err(
            &["31-"],
            N,
            "range 31-: start 31 exceeds file length (30 lines)",
        );
        err(
            &["8-999"],
            N,
            "range 8-999: end 999 exceeds file length (30 lines)",
        );
    }

    #[test]
    fn pool_order_first_failure_wins() {
        // 5-10 is valid; 0-2 fails (start < 1). 0-2 is reported.
        err(&["5-10,0-2"], N, "range 0-2: start must be at least 1");
    }

    #[test]
    fn empty_file() {
        // any range with start >= 1 exceeds a 0-line file
        err(
            &["1-"],
            0,
            "range 1-: start 1 exceeds file length (0 lines)",
        );
        // start 0 still reports the smaller-start error (check order)
        err(&["0-"], 0, "range 0-: start must be at least 1");
    }

    #[test]
    fn huge_numbers_do_not_panic() {
        let big = "9999999999999999999999999";
        let specs = vec![format!("{big}-")];
        let want = format!("range {big}-: start {big} exceeds file length ({N} lines)");
        let got = parse(&specs, N).unwrap_err().message();
        assert_eq!(got, want);
    }
}
