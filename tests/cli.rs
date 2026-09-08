//! Binary-level integration tests (§11.5 range outputs, §9 error templates,
//! §12.10 exit codes, §10 edge cases). TOC golden outputs are also asserted here
//! end-to-end (the library-level copy lives in `tests/golden_toc.rs`).

use std::path::PathBuf;
use std::process::Output;

use assert_cmd::Command;

const FILE: &str = "tests/fixtures/example.md";
const FIX: &str = "tests/fixtures";

fn cmd() -> Command {
    Command::cargo_bin("skimmd").unwrap()
}

fn run(args: &[&str]) -> Output {
    cmd().args(args).output().unwrap()
}

fn bytes(p: &str) -> Vec<u8> {
    std::fs::read(p).unwrap()
}

fn expect_err(args: &[&str], code: i32, stderr: &str) {
    let o = run(args);
    assert_eq!(o.status.code(), Some(code), "args={args:?}");
    assert!(
        o.stdout.is_empty(),
        "stdout must be empty on error, got: {:?}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert_eq!(String::from_utf8_lossy(&o.stderr), stderr, "args={args:?}");
}

// --- version / help ----------------------------------------------------------

#[test]
fn version_smoke() {
    let o = cmd().arg("--version").output().unwrap();
    assert!(o.status.success());
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(
        s.contains(env!("CARGO_PKG_VERSION")),
        "version line missing: {s}"
    );
}

#[test]
fn help_shows_range_grammar() {
    let o = cmd().arg("--help").output().unwrap();
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(
        s.contains("N-M") && s.contains("N-"),
        "range grammar missing: {s}"
    );
}

// --- TOC golden (end-to-end) -------------------------------------------------

#[test]
fn toc_md_golden() {
    let o = run(&[FILE]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(o.stdout, bytes("tests/fixtures/expected_toc.md"));
}

// --- TOC substring filter (-F / --filter) -----------------------------------

#[test]
fn filter_matches_title_and_body_case_insensitive() {
    // "install" is in the Install heading AND its body (`cargo install`).
    let o = run(&[FILE, "-F", "install"]);
    assert_eq!(o.status.code(), Some(0));
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("| 12 | 2 | 19 | 23 | Install |"), "{s}");
    assert_eq!(s.lines().count(), 3, "header + 1 row, got: {s}");
}

#[test]
fn filter_matches_body_only_keyword() {
    // "cargo" is only in the Install body, not any title -> proves body search.
    let o = run(&[FILE, "-F", "cargo"]);
    assert_eq!(o.status.code(), Some(0));
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("| 12 | 2 | 19 | 23 | Install |"), "{s}");
    assert_eq!(s.lines().count(), 3, "header + 1 row, got: {s}");
}

#[test]
fn filter_can_match_multiple_rows() {
    // "body" appears in the Usage body and the Setext body -> two rows.
    let o = run(&[FILE, "-F", "body"]);
    assert_eq!(o.status.code(), Some(0));
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("| 20 | 2 | 23 | 17 | Usage \\| Notes |"), "{s}");
    assert!(s.contains("| 24 | 2 | 27 | 17 | Setext Heading |"), "{s}");
    assert_eq!(s.lines().count(), 4, "header + 2 rows, got: {s}");
}

#[test]
fn filter_drops_unmatched_sections() {
    // "brew" matches only the macOS section; Install must be filtered out.
    let o = run(&[FILE, "-F", "brew"]);
    assert_eq!(o.status.code(), Some(0));
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("macOS"), "{s}");
    assert!(
        !s.contains("Install"),
        "unmatched sections must be filtered: {s}"
    );
}

#[test]
fn filter_no_match_is_empty_toc_and_exit_zero() {
    let o = run(&[FILE, "-F", "zzzz-no-such-word"]);
    assert_eq!(o.status.code(), Some(0), "no match is not an error");
    assert_eq!(
        o.stdout, b"| line | level | end | chars | title |\n|---|---|---|---|---|\n",
        "header only on no match"
    );
}

// --- range mode (§11.5) ------------------------------------------------------

#[test]
fn range_12_19() {
    let o = run(&[FILE, "12-19"]);
    assert_eq!(o.stdout, bytes("tests/fixtures/expected_12-19.txt"));
}

#[test]
fn range_1_4() {
    let o = run(&[FILE, "1-4"]);
    assert_eq!(o.stdout, bytes("tests/fixtures/expected_1-4.txt"));
}

#[test]
fn range_29_to_end() {
    let o = run(&[FILE, "29-"]);
    assert_eq!(o.stdout, bytes("tests/fixtures/expected_29-.txt"));
}

#[test]
fn range_8_8_and_16_to_end() {
    // two spec args
    let o = run(&[FILE, "8-8", "16-"]);
    assert_eq!(o.stdout, bytes("tests/fixtures/expected_8-8_16-.txt"));
    // equivalent comma form
    let o2 = run(&[FILE, "8-8,16-"]);
    assert_eq!(o2.stdout, o.stdout);
}

#[test]
fn range_1_minus_reproduces_file() {
    let o = run(&[FILE, "1-"]);
    assert_eq!(
        o.stdout,
        bytes(FILE),
        "1- must be byte-identical to the file"
    );
}

#[test]
fn range_merge_is_equivalent() {
    let a = run(&[FILE, "5-10,3-6"]);
    let b = run(&[FILE, "3-10"]);
    assert_eq!(a.stdout, b.stdout);
}

// --- range errors (§9, exit 1, no stdout) ------------------------------------

#[test]
fn range_errors() {
    expect_err(
        &[FILE, "31-"],
        1,
        "skimmd: range 31-: start 31 exceeds file length (30 lines)\n",
    );
    expect_err(
        &[FILE, "10-5"],
        1,
        "skimmd: range 10-5: end is less than start\n",
    );
    expect_err(
        &[FILE, "5"],
        1,
        "skimmd: invalid range spec '5': expected N-M or N- (comma-separated)\n",
    );
    expect_err(
        &[FILE, "1-10,x-y"],
        1,
        "skimmd: invalid range spec '1-10,x-y': expected N-M or N- (comma-separated)\n",
    );
    expect_err(
        &[FILE, "0-"],
        1,
        "skimmd: range 0-: start must be at least 1\n",
    );
    // first failing range in pool order
    expect_err(
        &[FILE, "5-10,0-2"],
        1,
        "skimmd: range 0-2: start must be at least 1\n",
    );
}

// --- file errors (§9, exit 1) ------------------------------------------------

#[test]
fn file_not_found() {
    expect_err(
        &["/nonexistent/skimmd_test_nope.md"],
        1,
        "skimmd: /nonexistent/skimmd_test_nope.md: No such file or directory\n",
    );
}

#[test]
fn file_is_a_directory() {
    expect_err(&[FIX], 1, &format!("skimmd: {FIX}: is a directory\n"));
}

#[test]
fn file_not_utf8() {
    let p = tmp_file("skimmd_not_utf8.bin", &[0xFF, 0xFE, 0x00, 0x80]);
    expect_err(
        &[p.to_str().unwrap()],
        1,
        &format!("skimmd: {}: not valid UTF-8\n", p.display()),
    );
}

#[test]
fn bom_is_invisible() {
    let p = tmp_file("skimmd_bom.md", b"\xef\xbb\xbf# Hello\nbody\n");
    let o = run(&[p.to_str().unwrap()]);
    // BOM stripped: a normal H1 at line 1, no phantom bytes.
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("| 1 | 1 | 2 | 5 | Hello |"), "{s}");
}

// --- stdin (-) ---------------------------------------------------------------

#[test]
fn no_args_is_same_as_dash_stdin() {
    // No FILE (or `-`) reads the TOC from standard input; the two must match byte-for-byte.
    let md = "# Hello\nbody\n## Sub\ntext\n";
    let a = cmd().write_stdin(md).output().unwrap();
    let b = cmd().arg("-").write_stdin(md).output().unwrap();
    assert_eq!(a.status.code(), Some(0), "no-args must succeed");
    assert_eq!(a.stdout, b.stdout, "no-args must behave like '-'");
    let s = String::from_utf8_lossy(&a.stdout);
    assert!(s.contains("| 1 | 1 | 4 | 5 | Hello |"), "TOC missing: {s}");
}

#[test]
fn stdin_toc() {
    let o = cmd()
        .arg("-")
        .write_stdin("# Hello\nbody\n## Sub\ntext\n")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("| 1 | 1 | 4 | 5 | Hello |"), "{s}");
    assert!(s.contains("| 3 | 2 | 4 | 5 | Sub |"), "{s}");
}

#[test]
fn stdin_range_mode() {
    let o = cmd()
        .args(["-", "2-3"])
        .write_stdin("l1\nl2\nl3\nl4\n")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(o.stdout, b"l2\nl3\n");
}

#[test]
fn stdin_1_minus_reproduces_input() {
    let md = "a\nb\n";
    let o = cmd().args(["-", "1-"]).write_stdin(md).output().unwrap();
    assert_eq!(o.stdout, md.as_bytes());
}

#[test]
fn stdin_bom_is_invisible() {
    let o = cmd()
        .arg("-")
        .write_stdin("\u{feff}# Hi\nx\n")
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&o.stdout);
    assert!(s.contains("| 1 | 1 | 2 | 2 | Hi |"), "{s}");
}

#[test]
fn stdin_not_utf8() {
    let o = cmd()
        .arg("-")
        .write_stdin([0xFF, 0xFE, 0x00])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(o.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&o.stderr),
        "skimmd: <stdin>: not valid UTF-8\n"
    );
}

// --- usage errors (clap, exit 2) ---------------------------------------------

#[test]
fn unknown_flag_is_usage_error() {
    let o = run(&["--bogus", FILE]);
    assert_eq!(o.status.code(), Some(2));
}

// --- broken pipe / stdout errors (§8.4, §10) ---------------------------------

#[test]
fn broken_pipe_is_clean_exit_zero() {
    let bin = cmd().get_program().to_string_lossy().into_owned();
    // `head -n 1` closes the pipe early; skimmd must exit 0 (not SIGPIPE) quietly.
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!(
            "{bin} {FILE} 1- | head -n 1 >/dev/null; echo \"${{PIPESTATUS[0]}}\""
        ))
        .output()
        .unwrap();
    let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        code,
        "0",
        "skimmd exit on EPIPE must be 0; stderr={:?}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn non_epipe_stdout_error_is_exit_one() {
    // /dev/full makes writes fail with ENOSPC (not EPIPE) -> exit 1 with a message.
    if !std::path::Path::new("/dev/full").exists() {
        return; // non-Linux: skip
    }
    let bin = cmd().get_program().to_string_lossy().into_owned();
    let out = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!("{bin} {FILE} 1- > /dev/full 2>/tmp/skimmd_nospc.$$; ec=$?; cat /tmp/skimmd_nospc.$$; rm -f /tmp/skimmd_nospc.$$; echo \"EC=$ec\""))
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("EC=1"), "expected exit 1; got: {s}");
    assert!(
        s.contains("skimmd: error writing to stdout:"),
        "expected stdout error message; got: {s}"
    );
}

/// Write `data` to a temp file and return its path (cleaned up on drop is not
/// guaranteed; tests use unique names).
fn tmp_file(name: &str, data: &[u8]) -> PathBuf {
    let p = std::env::temp_dir().join(name);
    std::fs::write(&p, data).unwrap();
    p
}
