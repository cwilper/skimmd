//! `skimmd` — command-line entry point. Phase 0 stub: just validates the clap
//! skeleton and prints the version. Real mode dispatch lands in Phase 4.

use clap::Parser;

/// skimmd: TOC and line-range view of a Markdown file.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// Path to a Markdown file (first positional is always the file).
    file: String,
}

fn main() {
    // Phase 0: parse to confirm the skeleton; a real file arg is required.
    let _cli = Cli::parse();
    eprintln!("skimmd {} (Phase 0 stub)", env!("CARGO_PKG_VERSION"));
}
