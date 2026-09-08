# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-09-08

### Added

- Read from standard input: `skimmd -` or `skimmd` (with `FILE` omitted) reads the
  file from stdin in both TOC and range modes.
- `-f, --filter <KEYWORD>` — filter TOC rows by a case- and whitespace-insensitive
  substring match on a section's heading or body (runs of whitespace, incl. newlines,
  collapse to one space; TOC mode only). No match prints an empty TOC and exits `0`.

### Changed

- TOC output is now always Markdown.

### Removed

- `--format` / `-f` (`md` / `tsv` / `json`). Markdown — formerly the default — is
  now the only TOC format. Note: the `-f` short flag previously meant `--format`;
  it now means `--filter`.

## [0.1.0] - 2026-09-02

### Added

- TOC mode: print a Markdown file's structure as a table of
  `line, level, end, chars, title` (leading text before the first heading is a
  level-0 `preamble` row).
- Range mode: `skimmd FILE RANGE...` prints the requested line ranges verbatim —
  `N-M`, `N-` (through the last line), comma- and/or space-separated, merged and
  normalized.
- `--format` / `-f` for TOC output: `md` (default), `tsv`, or `json`.
- Exit codes: `0` success (also a broken pipe), `1` request could not be satisfied
  (file / range / non-`EPIPE` stdout error), `2` usage error.
