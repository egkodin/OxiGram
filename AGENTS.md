# Working on OxiGram

## Start here

- Check `git status --short` and read the relevant entries in [PROGRESS.md](PROGRESS.md). Preserve existing work and keep the diff scoped to the request.
- Read the relevant [ADRs](docs/adr/) before changing a crate boundary, native dependency, or upstream pin.

## Write maintainable Rust

- Reuse existing types and helpers. Name values after what they represent, keep visibility narrow, and extract a helper when it gives a rule a clear name or removes duplication. Comments should explain invariants or upstream constraints.
- Use typed IDs and requests across account and TDLib boundaries. Propagate recoverable errors with context and keep credentials, session data, and message contents out of logs and test fixtures.
- Keep blocking I/O and decoding off the GPUI thread. Preserve per-account update order, reply routing, and clean shutdown when changing async code.
- Use the existing UI components, theme tokens, and localization. Preserve the current appearance and behavior unless the task changes them. Use the project generators for schema, style, and localization outputs.
- Keep shared code portable across Linux, Windows, and macOS. Isolate platform-specific code and verify it with the corresponding build configuration.

## Verify and hand off

- Reproduce bugs with an offline fixture where practical. Test the affected entry point and add a regression test when existing coverage would miss the failure. Use a live Telegram account only with explicit authorization.
- Use the native setup in [README.md](README.md#build-from-source) and the platform features in [CI](.github/workflows/ci.yml). For Rust changes, run formatting, the relevant tests, and strict Clippy. Run `cargo audit` and `cargo deny check` for dependency changes or release verification, and report unresolved advisories.
- Keep `target/` below 5 GB. Account for temporary build directories and clean up disposable outputs you created. Preserve existing release bundles and local credentials.
- Record significant results and remaining work in [PROGRESS.md](PROGRESS.md). Put comparable performance measurements in [docs/perf.md](docs/perf.md) and label offline runs. For parity changes, update [docs/parity.md](docs/parity.md) and run `scripts/update-roadmap.sh`.
- Report which platforms and paths were actually verified. Keep requested commits focused. Push or publish only when explicitly authorized.
