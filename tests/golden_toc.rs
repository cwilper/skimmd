//! Golden TOC verification at the library level: `build_toc` + `render` must be
//! byte-identical to the committed expected files (§11.2–11.4). This runs the
//! core without the CLI (the binary-level check lands in `tests/cli.rs`).

use skimmd::format::{Format, render};
use skimmd::lines::{LineMap, load_text};
use skimmd::toc::build_toc;

const FIX: &str = "tests/fixtures";

fn run(fmt: Format) -> Vec<u8> {
    let text = load_text(std::path::Path::new(&format!("{FIX}/example.md"))).unwrap();
    let lm = LineMap::new(text);
    let rows = build_toc(&lm);
    render(&rows, lm.n(), fmt).into_bytes()
}

fn expect(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIX}/{name}")).unwrap()
}

#[test]
fn md_matches_golden() {
    assert_eq!(run(Format::Md), expect("expected_toc.md"));
}

#[test]
fn tsv_matches_golden() {
    assert_eq!(run(Format::Tsv), expect("expected_toc.tsv"));
}

#[test]
fn json_matches_golden() {
    assert_eq!(run(Format::Json), expect("expected_toc.json"));
}

/// Cheap integrity canary for the fixture (the byte-compare above is the real
/// guard: any change to `example.md` changes the TOC and fails these tests).
#[test]
fn fixture_is_283_bytes() {
    let bytes = std::fs::read(format!("{FIX}/example.md")).unwrap();
    assert_eq!(bytes.len(), 283);
}
