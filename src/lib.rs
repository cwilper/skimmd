//! skimmd — agent-driven navigation of a single Markdown file.
//!
//! The library exposes the core operations; the `skimmd` binary (in `src/main.rs`)
//! is a thin CLI over them:
//!
//! * [`lines`] — the 1-based physical line model (`starts`, `N`, `line_of`, `span`, `chars`).
//! * [`ranges`] — range-spec grammar: parse, validate, normalize.
//! * [`elide`] — data-URL image elision for range output.
//! * [`toc`] — heading extraction and TOC row computation.
//! * [`format`] — the Markdown TOC emitter.

pub mod elide;
pub mod format;
pub mod lines;
pub mod ranges;
pub mod toc;
