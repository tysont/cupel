# Cupel

Cupel is a verification suite that grades what a coding agent produced. Functional tests tell you whether code works; Cupel adds the other ways of knowing — structural metrics, test-suite strength, mutation testing, generative testing, concurrency checks, formal specs, performance regression — behind one CLI with one output contract. An agent can learn the tool from the tool, run anything without side effects first, and always get back a ranked list of what to do next.

A *cupel* is the porous cup used in fire assay to separate precious metal from base.

## Status

Cupel is in the design and planning stage. No binary exists yet.

- [Technical design](docs/CUPEL_TECHNICAL_DESIGN.md) — the specification: CLI, output contract, check catalog, architecture.
- [Implementation plan](docs/CUPEL_IMPLEMENTATION_PLAN.md) — the phased, test-driven build plan, kept current as work proceeds.
- `docs/adr/` — architecture decision records written as implementation raises questions the design does not answer.

This README describes only functionality that exists. It grows with each completed phase of the plan.

## Principles

- **One binary, zero runtime dependencies.** Drops into any repository an agent is working in.
- **One output envelope.** Every verb returns the same JSON shape; every finding carries a stable ID, a severity, an evidence tier, a concrete action, and a reproduction command.
- **Self-describing.** Every verb, flag, check, finding kind, exit code, and schema is documented by the binary itself.
- **Side-effect free by default.** `--dry-run` on every verb; nothing is written outside `.cupel/` without `--write`.
- **Cupel grades; the agent fixes.** No fix mode, no interactive mode, no dashboard.

## License

BSD 3-Clause. See [LICENSE](LICENSE).
