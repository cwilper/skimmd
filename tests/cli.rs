//! Integration tests (grow per phase). Phase 0: a smoke test that the binary
//! builds and reports its version.

use assert_cmd::Command;

fn cmd() -> Command {
    Command::cargo_bin("skimmd").unwrap()
}

#[test]
fn version_smoke() {
    let out = cmd().arg("--version").output().unwrap();
    assert!(out.status.success(), "expected success, got {:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "version line missing: {stdout}"
    );
}
