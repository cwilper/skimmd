//! §12.8(b)/(c) binary-level range checks and §12.9 no-panic fuzzing.
//!
//! (The pure-logic §12.8 accounting/structural invariants live in `src/toc.rs`'s
//! test module, where they can use the private heading extractor directly.)

use skimmd::format::{Format, render};
use skimmd::lines::{LineMap, load_text};
use skimmd::ranges;
use skimmd::toc::build_toc;

const FILE: &str = "tests/fixtures/example.md";

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
