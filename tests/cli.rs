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

#[test]
fn toc_tsv_golden() {
    let o = run(&[FILE, "--format", "tsv"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(o.stdout, bytes("tests/fixtures/expected_toc.tsv"));
}

#[test]
fn toc_json_golden() {
    let o = run(&[FILE, "--format", "json"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(o.stdout, bytes("tests/fixtures/expected_toc.json"));
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

// --- usage errors (clap, exit 2) ---------------------------------------------

#[test]
fn missing_file_is_usage_error() {
    let o = run(&[]);
    assert_eq!(o.status.code(), Some(2));
}

#[test]
fn bad_format_is_usage_error() {
    let o = run(&[FILE, "--format", "xml"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
}

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
