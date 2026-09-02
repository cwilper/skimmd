# skimmd — Implementation Progress Log

Append-only running log. Re-read this + `docs/impl-plan.md` if I get confused.

## Setup / recon (before Phase 0)

- Read full spec `docs/skimmd-spec.md`. It is a complete, self-consistent v1 contract.
- Environment: `rustc`/`cargo` were NOT on PATH. The earlier `mise x rust` one-shot had run
  `rustup-init` but the toolchain download was interrupted. **Fixed:** completed
  `rustup toolchain install 1.98.0` (default toolchain). `~/.cargo/bin` on PATH gives
  cargo/rustc/clippy/rustfmt. (mise did not persist a rust tool; rustup did.)
- **Golden fixtures already provided** in `docs/golden-fixtures/`; verified every SHA-256
  against §11 (all match). `example.md` = 283 B, `N=30`.
- **Owner decision (my one question):** license = **Apache-2.0** (single).
- Plan committed: `docs/impl-plan.md`.
- Hand-verified the core algorithm against §11 numbers before coding (preamble `1-7/74`;
  ends `30,19,19,23,27,28,30`; body chars `17,23,14,17,17,0,26`; invariant `74+95+114=283`).
  All match → confidence the plan's algorithm is correct.

### Decisions / findings
- Use **lib + bin** (deviation from §13 suggestion) so the crate is wrappable in Rust later.
- Add **serde (derive)** with serde_json so `json` keys stay in load-bearing order.
- Fixtures copied to `tests/fixtures/` (self-contained tests); `docs/golden-fixtures/` stays as reference.
- Keep timeouts short on install/poll commands (owner feedback).

## Phase 0 — Scaffold & dependency probe
- [ ] in progress
