//! §12.8 accounting/structural invariants and §12.9 no-panic fuzzing.
//!
//! The accounting invariant (§6.2) is the strong correctness check on the TOC
//! engine: the preamble region, every heading's own lines, and every heading
//! row's body chars must partition the whole file.

use skimmd::format::{Format, render};
use skimmd::lines::{LineMap, load_text};
use skimmd::ranges;
use skimmd::toc::{build_toc, heading_spans};

const FILE: &str = "tests/fixtures/example.md";

/// A spread of in-memory texts covering the edge cases (§10).
fn corpus() -> Vec<&'static str> {
    vec![
        "",                               // empty
        "# H\n",                          // heading only, no preamble
        "plain\n",                        // no headings
        "---\ntitle: x\n---\n\n# Real\n", // setext trap (no phantom H1)
        "Alpha\nBeta\n=====\n",           // setext multi-line
        "a\r\nb\r\nc\r\n",                // CRLF
        "# A\n# B\n",                     // multiple H1
        "> ## Q\n- ## L\n",               // blockquote / list (not TOC)
        "## x {#id}\nbody\n",             // attributes stripped
        "#\n",                            // empty H1
        "no trailing newline",            // no final \n
        "# T\n\nbody\n",                  // simple
    ]
}

#[test]
fn accounting_invariant_holds() {
    for (i, text) in corpus().into_iter().enumerate() {
        let lm = LineMap::new(text.to_string());
        let rows = build_toc(&lm);
        let spans = heading_spans(&lm);
        let n = lm.n();
        let total = lm.text().chars().count();

        // preamble end P = (first heading's first_line - 1) if any, else N.
        let p = spans.first().map_or(n, |(fl, _)| *fl - 1);
        let mut sum = lm.chars(1, p); // 0 when p == 0 (suppressed preamble)
        for (fl, ll) in &spans {
            sum += lm.chars(*fl, *ll); // each heading's own lines
        }
        for r in rows.iter().filter(|r| r.level > 0) {
            sum += r.chars; // each heading row's body
        }
        assert_eq!(
            sum, total,
            "accounting invariant broken on corpus[{i}] (n={n}): {text:?}"
        );
    }
}

#[test]
fn last_row_end_equals_n() {
    for (i, text) in corpus().into_iter().enumerate() {
        let lm = LineMap::new(text.to_string());
        let rows = build_toc(&lm);
        let n = lm.n();
        if n == 0 {
            assert!(rows.is_empty(), "corpus[{i}] should have no rows");
        } else {
            assert_eq!(
                rows.last().map(|r| r.end),
                Some(n),
                "last row end != N on corpus[{i}]: {text:?}"
            );
        }
    }
}

#[test]
fn preamble_row_consistency() {
    // When the preamble row is present, it starts at 1 with level 0, end == first
    // heading's line - 1 (or N), and its title is "preamble".
    for text in ["intro line\n# Head\n", "# Head\n", ""] {
        let lm = LineMap::new(text.to_string());
        let rows = build_toc(&lm);
        let n = lm.n();
        if n == 0 {
            continue;
        }
        let first = &rows[0];
        assert_eq!(first.line, 1);
        if rows.len() > 1 {
            // preamble present
            assert_eq!(first.level, 0);
            assert_eq!(first.title, "preamble");
            assert_eq!(first.end, rows[1].line - 1);
        } else {
            // single row: a heading at line 1 (preamble suppressed)
            assert!(first.level > 0 && first.line == 1, "{text:?}");
        }
    }
}

/// §12.8(b): for every TOC row of the golden fixture, `skimmd FILE line-end`
/// succeeds (exit 0) with non-empty output.
#[test]
fn every_row_range_succeeds() {
    let text = std::fs::read_to_string(FILE).unwrap();
    let lm = LineMap::new(text);
    let rows = build_toc(&lm);
    for r in &rows {
        let out = assert_cmd::Command::cargo_bin("skimmd")
            .unwrap()
            .args([FILE, &format!("{}-{}", r.line, r.end)])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "row {}/{} failed: {r:?} stderr={:?}",
            r.line,
            r.end,
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(!out.stdout.is_empty());
    }
}

/// §12.8(c): `skimmd FILE 1-` reproduces the file bytes for the fixture.
#[test]
fn one_to_end_reproduces_file() {
    let out = assert_cmd::Command::cargo_bin("skimmd")
        .unwrap()
        .args([FILE, "1-"])
        .output()
        .unwrap();
    assert_eq!(out.stdout, std::fs::read(FILE).unwrap());
}

/// §12.9: fuzz TOC + range parsing with pseudo-random inputs; any panic fails
/// the test. Deterministic (seeded LCG) so the suite is reproducible.
#[test]
fn fuzz_no_panic() {
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u8
    };
    // Tricky charset: heading markers, emphasis, pipes, code, links, math-ish,
    // setext, CRLF, multibyte.
    const CHARSET: &[u8] = b" #*_~|`[]{}$<>-=\n\r123\xC3\xA9";

    for _ in 0..1000 {
        let len = (next() as usize) % 80;
        let mut buf = Vec::with_capacity(len + 3);
        if next() % 8 == 0 {
            buf.extend_from_slice(b"\xEF\xBB\xBF"); // BOM
        }
        for _ in 0..len {
            if next() % 4 == 0 {
                buf.push(next()); // raw random byte (often invalid UTF-8)
            } else {
                buf.push(CHARSET[next() as usize % CHARSET.len()]);
            }
        }

        // Mirror the real pipeline: UTF-8 validate + BOM strip (load_text), then
        // LineMap -> build_toc -> render. Invalid UTF-8 is skipped (load_text
        // would reject it); valid text must never panic.
        if let Ok(text) = String::from_utf8(buf) {
            let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
            let lm = LineMap::new(text);
            let rows = build_toc(&lm);
            // invariant still holds on fuzzed input
            let spans = heading_spans(&lm);
            let p = spans.first().map_or(lm.n(), |(fl, _)| *fl - 1);
            let mut sum = lm.chars(1, p);
            for (fl, ll) in &spans {
                sum += lm.chars(*fl, *ll);
            }
            for r in rows.iter().filter(|r| r.level > 0) {
                sum += r.chars;
            }
            assert_eq!(sum, lm.text().chars().count(), "fuzz invariant broken");
            let _ = render(&rows, lm.n(), Format::Md);
            let _ = render(&rows, lm.n(), Format::Tsv);
            let _ = render(&rows, lm.n(), Format::Json);

            // Fuzz the range parser too: random spec strings, random line counts.
            for _ in 0..10 {
                let n = lm.n();
                let spec_len = (next() as usize) % 12;
                let digits: &[u8] = b"0123456789-,";
                let spec: String = (0..spec_len)
                    .map(|_| digits[next() as usize % digits.len()] as char)
                    .collect();
                let _ = ranges::parse(&[spec], n); // must not panic
            }
        }
    }
}

/// §12.9 (end-to-end): write random bytes to a temp file and run TOC mode; it
/// must exit 0 (valid) or 1 (invalid UTF-8), never a panic (exit 101).
#[test]
fn fuzz_binary_no_panic() {
    let mut state: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let mut next = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (state >> 33) as u8
    };
    let charset = b" #*_~|`[]{}$-=\n\r\xC3\xA9";
    for trial in 0..30 {
        let len = (next() as usize) % 40;
        let mut buf = Vec::new();
        for _ in 0..len {
            buf.push(if next() % 3 == 0 {
                next()
            } else {
                charset[next() as usize % charset.len()]
            });
        }
        let path = std::env::temp_dir().join(format!("skimmd_fuzz_{trial}.md"));
        std::fs::write(&path, &buf).unwrap();
        let out = assert_cmd::Command::cargo_bin("skimmd")
            .unwrap()
            .arg(&path)
            .output()
            .unwrap();
        let code = out.status.code().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            code == 0 || code == 1,
            "trial {trial}: unexpected exit {code} (panic=101); stderr={stderr}"
        );
        assert!(
            !stderr.contains("panicked"),
            "trial {trial} panicked: {stderr}"
        );
        let _ = load_text(&path); // also exercise the lib loader directly
        std::fs::remove_file(&path).ok();
    }
}
