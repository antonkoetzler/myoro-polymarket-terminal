# Testing Policy

We test what is important and should be tested.

## Principles

- Test critical behavior and contracts first.
- Keep tests small, fast, and deterministic.
- Prefer unit tests for pure logic and state transitions.
- Add integration tests only when behavior crosses module boundaries and cannot be validated well with unit tests.
- Do not chase 100% coverage. Prioritize correctness for risky paths (execution, sizing, persistence, parsing, config).

## Scope Guidance

- Required: config load/save behavior, trade sizing logic, execution decision flow, and local persistence format.
- Required: regressions for bugs that were fixed.
- Optional: heavy UI rendering snapshots; add only when they protect meaningful behavior and stay maintainable.

## Done Criteria

- `cargo test` passes.
- New behavior includes tests for the expected path and key failure/skip paths.
- Tests are readable and clearly tied to business behavior.
