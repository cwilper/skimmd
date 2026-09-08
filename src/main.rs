//! `skimmd` — command-line entry point.
//!
//! Two modes (spec §2):
//! * TOC mode — `skimmd [--format md|tsv|json] [FILE]` prints the structure table.
//!   (FILE omitted or `-` reads from standard input.)
//! * Range mode — `skimmd FILE RANGE...` prints the requested line ranges verbatim.
//!
//! Exit codes (spec §9): 0 success; 1 the request could not be satisfied
//! (file / range / stdout errors); 2 clap usage errors (handled by clap).

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use clap::Parser;
use skimmd::format::{Format, render};
use skimmd::lines::{LineMap, LoadErr, io_reason, load_stdin, load_text};
use skimmd::ranges;
use skimmd::toc::{build_toc, filter_rows};

/// skimmd: TOC and line-range view of a single Markdown file.
#[derive(Parser)]
#[command(
    version,
    about = "Print a Markdown file's structure as a TOC (line numbers and sizes), or read verbatim line ranges.",
    after_help = "RANGE grammar: N-M or N- (N- = through the last line). \
        Ranges are comma- and/or space-separated, e.g. 1-5,9-12 or 1-5 9-12. \
        With no ranges, the TOC is printed. \
        Omit FILE (or use -) to read from standard input. "
)]
struct Cli {
    /// TOC output format (ignored in range mode).
    #[arg(short, long, value_enum, default_value_t = Format::Md)]
    format: Format,

    /// Filter TOC rows by substring (TOC mode only, ignored in range mode).
    /// Case-insensitive; keeps a row if KEYWORD occurs in its heading or its
    /// section body.
    #[arg(short = 'F', long = "filter", value_name = "KEYWORD")]
    filter: Option<String>,

    /// Path to a Markdown file, or `-` for stdin. Omitted (or `-`) reads from stdin.
    #[arg(required = false)]
    file: Option<String>,

    /// Zero or more line ranges. Any range switches to range mode.
    #[arg(value_name = "RANGE")]
    ranges: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse(); // clap prints usage and exits 2 on usage errors.

    // Omitted FILE (or "-") reads from standard input.
    let (label, res) = match &cli.file {
        Some(f) if f != "-" => (f.as_str(), load_text(std::path::Path::new(f))),
        _ => ("<stdin>", load_stdin()),
    };
    let text = match res {
        Ok(t) => t,
        Err(LoadErr::Io(e)) => die(&format!("{label}: {}", io_reason(&e))),
        Err(LoadErr::NotUtf8) => die(&format!("{label}: not valid UTF-8")),
    };
    let lm = LineMap::new(text);

    if cli.ranges.is_empty() {
        toc_mode(&lm, cli.format, cli.filter.as_deref())
    } else {
        range_mode(&lm, &cli.ranges)
    }
}

/// TOC mode: compute rows, optionally filter them with `-F`, and render in the
/// requested format.
fn toc_mode(lm: &LineMap, format: Format, filter: Option<&str>) -> ExitCode {
    let rows = build_toc(lm);
    let rows = match filter {
        Some(kw) => filter_rows(lm, &rows, kw),
        None => rows,
    };
    write_stdout(&render(&rows, lm.n(), format))
}

/// Range mode: parse/validate/normalize all ranges **before any output**, then
/// write each merged range's bytes in order, with no separators.
fn range_mode(lm: &LineMap, specs: &[String]) -> ExitCode {
    let merged = match ranges::parse(specs, lm.n()) {
        Ok(m) => m,
        Err(e) => die(&e.message()),
    };
    let mut out = String::new();
    for r in &merged {
        out.push_str(lm.span(r.start, r.end));
    }
    write_stdout(&out)
}

/// Write `out` to stdout via a BufWriter (flushed once). `EPIPE` is a clean
/// exit 0 with no message; any other write error is exit 1 (spec §8.4/§9).
fn write_stdout(out: &str) -> ExitCode {
    let stdout = std::io::stdout();
    let mut w = BufWriter::new(stdout);
    match w.write_all(out.as_bytes()).and_then(|_| w.flush()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(e) => die(&format!("error writing to stdout: {e}")),
    }
}

/// Print `skimmd: {msg}` to stderr and exit 1.
#[cold]
fn die(msg: &str) -> ! {
    eprintln!("skimmd: {msg}");
    std::process::exit(1);
}
