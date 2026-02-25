# Rust standards

Follow the project's Rust standards document for all Rust code.

**Canonical reference:** [docs/standards/STANDARDS.md](../standards/STANDARDS.md)

Summary:

- **Uniform:** One preferred way per concern; no second style without a documented reason.
- **Anti-fragile:** Invalid state unrepresentable where possible; `Result`/`Option`; no panics in library code.
- **Scalable:** Shared traits and modules; domain code under `src/strategies/<domain>/`.
- **Flexible:** Config and behaviour via env and types; pluggable strategies and data sources.
- **Consistent:** Same error handling, naming, layout, and testing patterns everywhere.

When editing `.rs` files, apply these rules and the full doc.
