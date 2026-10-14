# Cupel — Implementation Plan

**Status:** Living document · **Created:** 2026-10-10 · **Design:** [CUPEL_TECHNICAL_DESIGN.md](CUPEL_TECHNICAL_DESIGN.md)

---

## 0. How to use this document

This is a **living document**. The agent implementing Cupel owns it and must keep it truthful.

1. **Mark status as you go.** Every phase and every step carries a status line. Update it the moment work starts and the moment it finishes:
   - `Status: not started`
   - `Status: in progress` — add the date started
   - `Status: complete` — add the date completed and the commit SHA that closed it
   - `Status: blocked` — add the reason and what unblocks it
2. **Leave notes.** Each phase has a `Notes` block. Record anything significant: deviations from the design, surprises, bugs found in earlier phases, performance numbers, flaky tests you had to investigate. Notes describe what *is*, not what used to be. Do not delete notes; append to them.
3. **Record decisions as ADRs.** Any time the design is silent, ambiguous, or wrong, and you have to choose, write `docs/adr/CUPEL_DECISION_XXX.md` (zero-padded, sequential: `001`, `002`, …) **before** writing the code that depends on it. Format in §2.4. Link the ADR from the step that triggered it. Pre-identified decisions are listed in §2.5 with reserved numbers.
4. **Do not reorder or skip steps** without recording why in `Notes`. Steps within a phase may be done in parallel only where marked `∥`.
5. **The design document is the specification.** Where this plan and the design disagree, the design wins unless an ADR says otherwise. Where both are silent, write an ADR.
6. **Update the README at the end of every phase** so it describes only what exists and works. Never describe unbuilt functionality as present.

### Status legend used below

| Marker | Meaning |
|---|---|
| `not started` | No code written |
| `in progress (YYYY-MM-DD)` | Work underway |
| `complete (YYYY-MM-DD, <sha>)` | Definition of done met, tests green, committed |
| `blocked (reason)` | Cannot proceed |

---

## 1. Strategy

### 1.1 Shape of the plan

The design's §12 lists phases P0–P8. This plan keeps that numbering so the two documents cross-reference cleanly, splits each phase into steps small enough to finish and verify in one sitting, and adds **P9 — Dogfood** as the explicit final milestone.

The phases fall into two kinds:

- **Platform milestones (P0, P1)** build broad, horizontal infrastructure: the workspace, the output contract, the CLI, configuration, the fixture/golden harness, and the language layer. Nothing user-visible "grades code" yet, but every piece is independently testable and every later slice stands on it.
- **Vertical slices (P2 onward)** each deliver one complete user-visible capability end-to-end — a check from manifest through `describe`, `plan`, `run`, findings, `explain`, text rendering, and golden envelope — before the next one starts. The first slice (P2.1, `static.complexity` through the `check` verb) is deliberately the thinnest possible path from `cupel check` to a real finding; it proves the whole pipeline before any breadth is added.

### 1.2 Validation per phase

Every phase ends with a **Definition of Done (DoD)** block. A phase is not complete until every DoD line holds. Common to every phase:

- `make check` passes (fmt, clippy with `-D warnings`, all tests, doc build).
- `git status` is clean and nothing is in the tree that shouldn't be (§2.6).
- README updated to reflect current functionality.
- Plan status and notes updated; ADRs written for any decisions.
- From P2.10 onward: Cupel's own gate (`cupel check --changed --gate`) passes on the Cupel repo.

### 1.3 Test-driven development — non-negotiable

For every step:

1. Write the test(s) that specify the behavior. For CLI behavior this is an integration test under `crates/cupel-cli/tests/`; for a check it is a fixture file plus an expected golden envelope; for a pure function it is a unit test.
2. Run the test. **Confirm it fails for the right reason** (not a compile error in the test itself, not a missing fixture). Record nothing; just look.
3. Write the minimum implementation that makes it pass.
4. Run the full suite. Refactor while green.
5. Commit.

Tests must never only validate mocked behavior. Producers that wrap external tools are tested against **real** tool output captured as fixtures (parsers) *and* against the real tool when it is present on the machine (integration, skipped with a visible `SKIPPED: <tool> not installed` reason, never silently).

Test output must be pristine: a passing run prints no warnings, no stray `println!`, no leftover files.

### 1.4 Test taxonomy

| Kind | Where | Tool | What it proves |
|---|---|---|---|
| Unit | `src/**` `#[cfg(test)]` | `cargo test` | Pure logic: hashing, scoring, normalization, config precedence, scheduler ordering |
| Property | `src/**` `#[cfg(test)]` | `proptest` | Invariants: structural hash is rename/reformat-invariant; schema round-trips; finding IDs stable under relocation |
| Snapshot | `tests/` and `src/**` | `insta` | JSON Schema output, `describe` output, text renderer output |
| CLI integration | `crates/cupel-cli/tests/` | `assert_cmd` + `predicates` | Verbs, flags, exit codes, stdout/stderr discipline, TTY vs pipe format |
| Golden envelope | `crates/cupel-fixtures/` | custom harness (§P0.6) | Each check against each language fixture produces the expected envelope modulo volatile fields |
| External-tool integration | `crates/cupel-adapt/tests/` | `assert_cmd`, real tools | Tool producers drive real `cargo test`, `pytest`, `vitest`, etc. when installed |
| Parity | `crates/cupel-serve/tests/` | custom | Cold CLI output == daemon-proxied output, byte for byte |
| Dogfood | CI job | `cupel` itself | Gate passes on the Cupel repo |

---

## 2. Project conventions

### 2.1 Repository layout

```
cupel/
├── Cargo.toml                 # workspace; shared [workspace.dependencies] and lints
├── Cargo.lock
├── rust-toolchain.toml        # pinned stable channel
├── Makefile                   # the only entry point humans and CI use
├── README.md                  # the ONLY markdown file outside docs/
├── LICENSE
├── .gitignore
├── .github/workflows/ci.yml
├── crates/
│   ├── cupel-cli/             # binary `cupel`: clap, renderers, exit codes, daemon auto-connect
│   ├── cupel-core/            # fact types + Fact/Producer/Check traits + registries + support matrix;
│   │                          #   Envelope/Finding types, schemas, scope, fact DAG scheduler, config
│   ├── cupel-lang/            # Language trait, generic query-driven producers, structural hashing, parse cache
│   │   └── src/langs/<id>/    # one dir per language: mod.rs + queries/*.scm + tables.toml (design §8.4.2)
│   ├── cupel-store/           # SQLite fact store, migrations, queries
│   ├── cupel-checks/          # native checks, one module per check; depends on cupel-core ONLY (see ADR-001)
│   ├── cupel-adapt/           # tool producers (runners, coverage, bench), one module per tool; toolchain table
│   ├── cupel-mutate/          # mutation engine: Producer<MutationOutcomes> (P3)
│   ├── cupel-plugin/          # plugin protocol types + external-process host for check and producer plugins (P5)
│   ├── cupel-serve/           # daemon, socket protocol, watcher, MCP (P4, P7)
│   └── cupel-fixtures/        # fixture repos + golden envelopes + harness (dev-dependency)
│       ├── conformance/<fact>.toml       # per-fact construct inventories every producer must cover (design §8.3.7)
│       ├── repos/<lang>/...   # known-bad source trees, each with inventory.toml mapping construct IDs to code
│       └── golden/<check>/<lang>.json    # one per supported (check, lang) pair — enumerated by the matrix test
└── docs/
    ├── CUPEL_TECHNICAL_DESIGN.md
    ├── CUPEL_IMPLEMENTATION_PLAN.md
    └── adr/
        └── CUPEL_DECISION_001.md ...
```

`cupel-checks` as a single crate (modules per check) is a deliberate deviation from the design's original `cupel-check-*` layout; see ADR-001 (§2.5). Split a check out into its own crate only when it acquires heavy, check-specific dependencies.

The crate graph is part of the design (design §8.2): `cupel-checks` → `cupel-core` only; `cupel-lang`, `cupel-adapt`, `cupel-mutate` → `cupel-core` only; `cupel-cli` → everything. A `Cargo.toml` change that violates this is a design change and needs an ADR.

### 2.2 Toolchain and dependencies

Pin a stable Rust channel in `rust-toolchain.toml` (1.92 or newer is available locally). Edition 2024. All dependency versions are declared once in `[workspace.dependencies]`.

Core crates (add only when a step needs them; never pre-add):

| Purpose | Crate |
|---|---|
| CLI | `clap` (derive) |
| Serialization | `serde`, `serde_json`, `toml` |
| JSON Schema | `schemars` |
| Hashing | `blake3` |
| Run IDs | `ulid` |
| Parsing | `tree-sitter`, `tree-sitter-javascript`, `tree-sitter-typescript` (typescript + tsx), `tree-sitter-rust`, `tree-sitter-go`, `tree-sitter-python` |
| Store | `rusqlite` (feature `bundled`) |
| Parallelism | `rayon`, `tokio` |
| File walk / watch | `ignore`, `notify` |
| TTY detection | `std::io::IsTerminal` |
| Errors | `thiserror` (libraries), `anyhow` (binary only) |
| Testing | `assert_cmd`, `predicates`, `insta`, `proptest`, `tempfile` |

### 2.3 Automation (`Makefile`)

All build, test, lint, and run steps go through `make`. Agents and CI never call `cargo` directly for these.

| Target | Does |
|---|---|
| `make build` | `cargo build --workspace` |
| `make test` | `cargo test --workspace` |
| `make lint` | `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` (with `disallowed_methods` forbidding `std::process` and `std::fs` in `cupel-checks`) |
| `make matrix` | `cargo run -- describe matrix --format text` — the current checks × languages support table |
| `make readme` | rewrites the README's generated sections (exit codes, check table, support matrix) from `cupel describe`; `make test` fails while they are stale ([ADR-073](adr/CUPEL_DECISION_073.md)) |
| `make fmt` | `cargo fmt` |
| `make doc` | `cargo doc --workspace --no-deps` with `-D warnings` on rustdoc |
| `make check` | `lint` + `test` + `doc` + `make clean-check` — the pre-commit bar |
| `make clean-check` | fails if `git status --porcelain` is non-empty after a build (catches generated files that escaped `.gitignore`) |
| `make dogfood` | `cargo run -- check --changed --gate` on this repo (from P2.10) |
| `make install` | `cargo install --path crates/cupel-cli` |

### 2.4 ADR format

`docs/adr/CUPEL_DECISION_XXX.md`:

```markdown
# CUPEL_DECISION_XXX: <short title>

**Date:** YYYY-MM-DD · **Status:** proposed | accepted | superseded by XXX · **Phase:** P<n>.<m>

## Context
What question had to be answered, and why the design/plan did not already answer it.

## Options
Each option considered, one paragraph each, with concrete trade-offs.

## Decision
What was chosen.

## Consequences
What this makes easier, harder, or impossible later. Any follow-up work it creates.
```

ADRs are immutable once accepted. To change a decision, write a new ADR that supersedes the old one and update the old one's status line only.

### 2.5 Pre-identified decisions

These must be written as ADRs at the step indicated. Numbers are reserved; additional ADRs continue from 010.

Two standing triggers also require an ADR whenever they occur, at whatever phase: **adding a fact kind** to the catalog in design §8.3.2 (it is the one change that costs across every language), and **changing the crate dependency graph** in §2.1.

| # | Decision | Write at | Guidance |
|---|---|---|---|
| 001 | Crate granularity: `cupel-checks` single crate vs `cupel-check-*` per check | P0.1 | Plan prescribes single crate; ADR records the reasoning and the split criterion |
| 002 | Git access: shell out to `git` vs `gix`/`git2` | P2.1 | Shelling out keeps the binary small and matches the user's git; a library gives speed in the daemon. Measure before choosing |
| 003 | Golden envelope redaction and comparison strategy | P0.6 | Which fields are volatile, how goldens are updated (`UPDATE_GOLDENS=1`), how diffs are reported |
| 004 | Equivalent-mutant handling in v1 (design open question 2) | P3.4 | Design leans toward emitting `mutant.equivalent?` with `confidence ≤ 0.3`, `severity: info` |
| 005 | Go schemata timing: P4 vs P6 (design open question 3) | P4.1 | Go language and fixtures exist from P1, so the generic-inference edge cases can be measured before deciding |
| 006 | Per-test coverage for JS/TS (design open question 1) | P3.3 | V8 `NODE_V8_COVERAGE` per test vs permanent file-level fallback |
| 007 | `spec.trace` instrumentation API (design open question 4) | P7.2 | Attribute/macro vs logging convention |
| 008 | `meta.traceability` requirement-ID source (design open question 5) | P7.6 | Spec file vs `#REQ-12` in test names vs both |
| 009 | Daemon socket protocol framing | P4.2 | Length-prefixed JSON vs JSONL vs something else |

### 2.6 Repository cleanliness

The repository must be pristine at every commit and at the end of every phase.

- **No documentation outside `docs/` except `README.md`.** No `CONTRIBUTING.md`, `CHANGELOG.md`, `TODO.md`, per-crate READMEs, or notes files. Crate-level documentation lives in `lib.rs` doc comments.
- **No scratch files.** Temporary scripts, experiments, and captured outputs go in the session scratchpad, never the repo. Fixture captures that become tests go under `crates/cupel-fixtures/` or a crate's `tests/fixtures/`.
- **`.gitignore` covers** `target/`, `.cupel/` (except `suppressions.toml`), `.DS_Store`, editor files. Add to it the moment a new generated artifact appears.
- **No commented-out code, no `#[allow(dead_code)]` without a comment explaining why, no `todo!()` or `unimplemented!()` in committed code** outside a step explicitly marked in progress — and none at phase completion.
- **Every source file starts with a two-line `// ABOUTME:` comment** describing its purpose.
- **Names are domain-focused.** No `New*`, `Legacy*`, `*Impl`, `*Wrapper`, `*Helper`. Match existing codebase style over external convention.
- **`make clean-check` is part of `make check`**: a build must not dirty the tree.
- Commit frequently with messages that describe behavior, not history. Each step is at least one commit. Never skip pre-commit hooks.

### 2.7 README standard

The README is the front door and must be world class at every phase boundary. It must:

- Open with what Cupel is, in two sentences, and the one-command quick start (`cupel init --write && cupel check --changed`).
- Show real output (a trimmed envelope and its `--format text` rendering) captured from the actual binary, refreshed whenever the shape changes.
- List verbs and checks that **exist**, with tier and cost, generated from `cupel describe --all` once that exists (P8.3) so it cannot drift.
- Explain the agent loop ("do not stop until `cupel check --changed --depth standard --gate` exits 0") and the exit-code table.
- Have Install, Usage, Configuration, Agents (`describe --agent`), Development (`make` targets), and License sections.
- Link to `docs/CUPEL_TECHNICAL_DESIGN.md` and `docs/CUPEL_IMPLEMENTATION_PLAN.md`.
- Contain no roadmap, no "coming soon", no screenshots of unbuilt features.

### 2.8 Matrix rule: checks × languages × tools

Cupel's three extension axes meet only through facts (design §8.3). Every step in this plan respects these rules, and the test suite enforces them mechanically:

- **Checks never see a language.** A check module declares `needs`/`wants` fact kinds and computes over `Facts`. `cupel-checks` depends on `cupel-core` alone, and clippy forbids `std::process`/`std::fs` inside it. If a check "needs to know" something about a language, the answer is always a semantic field on an existing fact or (with an ADR) a new fact — never a `match lang`.
- **Producers never see a check.** Language producers, tool producers, and engines emit facts and read only `[lang.*]` config. The mutation engine does not know what `tests.mutation` will do with its outcomes.
- **Language support is derived, never declared.** There is no capability struct. `describe matrix` is computed from check needs and registered producers; the README's matrix is generated from it (P8.3).
- **Five launch languages are peers.** `javascript`, `typescript`, `rust`, `go`, `python`. Every P1 step delivers its fact's producer for all five before the step is marked complete. Throughout this plan, "every language" means all five.
- **The matrix test is the arbiter.** It enumerates the registries: every `supported` (check, lang) pair must have a passing golden; every `unsupported` pair must have a passing `skipped` assertion; every check must pass when given only its declared facts. Nobody maintains a list of what works where.
- **Per-fact conformance.** Each fact kind owns an inventory in `cupel-fixtures/conformance/<fact>.toml`; a language's `inventory.toml` must map every construct or mark it `not-applicable` with a reason. An unmapped construct is a build failure.
- **Adding a language later** (Java and Clojure in P6) follows the design §8.4.4 checklist. **Adding a check later** is one module plus goldens. **Adding a tool** is one producer plus a toolchain-table cell. If any of these requires touching `cupel-core`, another axis's crate, or the CLI, stop: write an ADR, fix the extension model, then continue.

---

## 3. Phases

---

### P0 — Platform skeleton

**Goal:** A `cupel` binary with the complete output contract, CLI surface, configuration system, `describe`/`init`/`doctor` verbs, and a fixture/golden harness — but no language analysis yet. Everything later plugs into this.

**Status:** complete (2026-10-10, 13f7b14)

#### P0.1 Workspace, automation, CI

**Status:** complete (2026-10-10, de1ff35)

- Cargo workspace with `cupel-cli` and `cupel-core` crates; `rust-toolchain.toml`; `[workspace.lints]` enabling `clippy::all`, `clippy::pedantic` selectively, `unsafe_code = "forbid"` (relax per-crate only via ADR).
- `Makefile` per §2.3. `.github/workflows/ci.yml` running `make check` on Linux and macOS.
- `.gitignore` per §2.6.
- `cupel --version` prints `cupel <semver>`; `cupel` with no args prints usage to stderr and exits 2.
- Write **ADR-001**.

ADRs: [001](adr/CUPEL_DECISION_001.md) (crate granularity), [010](adr/CUPEL_DECISION_010.md) (`clean-check` semantics).

*Tests first:* `tests/cli_basics.rs` — version string shape, no-args exit code 2, unknown verb exit 2.

#### P0.2 Core types and schemas

**Status:** complete (2026-10-10, e3cea34)

In `cupel-core`, define the output contract exactly as design §7 specifies, deriving `Serialize`, `Deserialize`, `JsonSchema`:

- `Envelope`, `RunInfo`, `GitInfo`, `ScopeInfo`, `ToolchainInfo`, `ConfigEcho`, `BaselineRef`
- `Status` (`pass | fail | partial | error | plan`), `ExitCode` (0–6 with names from §6.6)
- `CheckResult`, `Score`, `Delta`
- `Finding`, `Severity`, `Symbol`, `Location`, `Action`, `ActionKind` (closed enum from §7.2), `Evidence` (tagged per kind)
- `Summary`, `GateResult`, `GateRule`, `NextAction`
- `Event` (all `ev` variants from §7.4)
- `FindingId::compute(check, kind, symbol_hash, site_hash)` = `fnd_` + first 24 bits of blake3 as hex (§7.2)
- `RunId` (ULID, `run_` prefix); `SymbolHash` (`sh_` prefix)
- The extension-model types from design §8.3: `FactKind` (closed enum, one variant per catalog row in §8.3.2), the `Fact` trait (`KIND`, `VERSION`), `Producer<F>` and `Check` traits, `Langs`, `Availability`, `Manifest` with `needs`/`wants`, `ProducerRegistry`, `CheckRegistry`, `Facts` accessor, and `SupportMatrix::compute(checks, producers, langs)` with `Support::{Supported, Unsupported(FactKind), Unavailable(ToolId)}`.
- Fact payload types for the **syntactic** facts (`Symbols`, `Decisions`, `NormTree`, `MutationSites`, `TestFrames`, `ModuleGraph`, `ApiSurface`, `Roots`, `TestConventions`, `ProjectInfo`), each with `JsonSchema`. Execution-fact payloads are added in the phase that first produces them, each with an ADR only if it is not already in the §8.3.2 catalog.

Schemas are generated by `schemars` and exposed via `cupel_core::schema::{run, finding, events, config, plugin, fact(kind)}()`.

ADRs: [011](adr/CUPEL_DECISION_011.md) (language identity in core), [012](adr/CUPEL_DECISION_012.md) (open evidence), [013](adr/CUPEL_DECISION_013.md) (output-contract details), [014](adr/CUPEL_DECISION_014.md) (syntactic payload shape), [015](adr/CUPEL_DECISION_015.md) (support-matrix resolution).

*Tests first:* serde round-trip for every type; `insta` snapshots of every schema; unit tests for `FindingId` (known vector, 24-bit truncation, prefix); property test that `FindingId` ignores line numbers (it takes only structural inputs by construction — test that the API makes it impossible to pass a line); `SupportMatrix::compute` unit tests on synthetic registries (a check needing `{A, B}` with producers for `A` only → `Unsupported(B)`; a producer whose `availability` is `MissingTool` → `Unavailable`); `Facts::get` of an undeclared kind is an error.

#### P0.3 CLI framework and `describe`

**Status:** complete (2026-10-10, 30c71a1)

- `clap` derive with every global flag from §6.3 on a shared struct; every verb from §6.2 declared (unimplemented verbs return exit 2 with a finding `kind: cli.not-implemented` and a `detail` naming the phase — remove this as verbs land).
- Format auto-detection: JSON when stdout is not a terminal, text when it is; `--format` overrides; `CUPEL_FORMAT` env respected.
- Exit-code mapping: every verb returns `Result<Envelope, CupelError>`; the binary converts to envelope + exit code. **Every** error path produces a valid envelope on stdout (usage → exit 2 with a finding naming the fix; internal → exit 6 with bug-report payload). stderr is only for `--verbose`/`--quiet`-controlled diagnostics and `--events stderr`.
- Self-description registry: a single data structure describing verbs, flags (type, default, enum values), exit codes, and schema URLs. **Both** clap help and `describe` render from it so they cannot drift.
- `cupel describe`, `cupel describe <verb>`, `cupel describe schema <name>`, `cupel describe --all`, `cupel <verb> --describe`. `describe <check-id>` and `describe finding <kind>` come online as checks land (P2+). `describe --agent` is P8.3.
- Every described entity carries `stability`.

*Tests first:* `tests/describe.rs` snapshots; `tests/format.rs` (pipe → JSON, `--format text` → text, invalid format → exit 2 envelope); `tests/exit_codes.rs` (each code reachable via a deliberate trigger); a test asserting clap's `--help` output mentions every flag the registry declares and vice versa.

#### P0.4 Configuration

**Status:** complete (2026-10-10, 411e22d)

- `cupel.toml` model per §6.7 with `serde` + `toml`; unknown keys at any level are usage errors (exit 2) listing valid keys.
- Discovery: `--config PATH`, else nearest `cupel.toml` walking up from `--cwd` (default: current dir); `--cwd` establishes repo root — **never** rely on process cwd elsewhere.
- Precedence: flags > `CUPEL_*` env > `--profile` > `cupel.toml` > defaults. Effective config and its sources echoed under `run.config`.
- Gate rule key validation (`<check-id>.<metric>` or `<check-id>`; operators `>= > <= < == none no-breaking`) — parsing only; evaluation is P2.9. Unknown check IDs error at parse time once the check registry exists (P2.1); until then, validate syntax only.

ADRs: [017](adr/CUPEL_DECISION_017.md) (layered settings, gate-key grammar, sources, override merge, registry validation).

*Tests first:* precedence matrix as a table-driven unit test; unknown-key error message snapshot; walking-up discovery with `tempfile`; profile merge.

#### P0.5 Renderers: JSON, JSONL, text

**Status:** complete (2026-10-10, d512992)

- `json`: pretty, stable key order (serde struct order), trailing newline.
- `jsonl`: header line (envelope minus `findings`), then one finding per line.
- `text`: per §7.3 — status line, gate result, findings grouped by severity then check, each with location, title, action summary, repro; `next_actions` last; evidence summarized to one line. `--no-color` and non-TTY disable color. No information in JSON is omitted except evidence bodies.
- `--limit`/`--offset` paging with `truncated` and `total_findings` always set.

ADRs: [019](adr/CUPEL_DECISION_019.md) (JSONL header shape, paging semantics, text rendering rules, color).

*Tests first:* snapshot each renderer against a hand-built envelope containing every field; paging arithmetic unit tests; property test that `jsonl` header + lines reassemble into the `json` envelope.

#### P0.6 Fixture and golden-envelope harness

**Status:** complete (2026-10-10, 9981a5f)

- `crates/cupel-fixtures` with `repos/` (populated per language from P1), `golden/`, and a library offering `run_cupel(args, fixture) -> Envelope` and `assert_golden(name, envelope)`.
- Redaction of volatile fields (`run.id`, `run.started`, all `duration_ms`, `toolchain` versions, absolute paths, `git.sha`) before comparison. `UPDATE_GOLDENS=1 make test` rewrites goldens; CI never sets it. Golden diffs are printed as unified diffs.
- Write **ADR-003**.

ADRs: [003](adr/CUPEL_DECISION_003.md) (volatile fields and placeholders, golden storage, `UPDATE_GOLDENS`, diff reporting).

*Tests first:* the harness tests itself — a trivial `describe` envelope round-trips through redaction; an intentionally changed field produces a readable diff.

#### P0.7 Run infrastructure: `.cupel/`, run IDs, events, timeout, dry-run plumbing

**Status:** complete (2026-10-10, efa7a3f)

- `.cupel/` directory conventions: `cupel.db` (P2.2), `runs/<id>.json`, `cache/`, `work/<run>/`, `.gitignore` written by `init`. Nothing is written outside `.cupel/` without `--write`; a central `Writer` type enforces this and is the only way to write files.
- `--events PATH|stderr` JSONL sink; stdout reserved for the envelope.
- `--timeout DURATION` (parse `10s`, `5m`, `1h`): cooperative cancellation token; on expiry, outstanding work is cancelled, `status: partial`, exit 5, partial results included.
- `--dry-run` produces a `plan` envelope (§6.8) with `would_run`, `would_skip`, `would_install`, `would_write`; for writing verbs `would_write` carries unified diffs.
- `--seed` recorded in `run.seed` (default: random, always echoed).

ADRs: [020](adr/CUPEL_DECISION_020.md) (the `Writer` and `.cupel/` layout, run records, the event stream, cancellation and the slow-task hook, the dry-run plan, seeds).

*Tests first:* `Writer` refuses paths outside `.cupel/` without `--write` (unit); events file contains `plan` … `run.end` for a trivial verb; `--timeout 1ms` on a verb with an injected slow task yields exit 5 and `partial`.

#### P0.8 `init` and `doctor`

**Status:** complete (2026-10-10, 68ac27e)

- `init`: detects project roots and languages (Cargo workspace, `go.work`/`go.mod`, npm/pnpm/yarn workspaces, uv/poetry/pip, Maven/Gradle, `deps.edn`) and the default test/coverage/bench commands per language (the detection table lives in `cupel-lang` once P1 lands; in P0 it is a standalone detection module that P1.9 moves). Writes `cupel.toml` and `.cupel/.gitignore` only with `--write`; `--dry-run` shows the file as a diff; refuses to overwrite without `--force`.
- `doctor`: binary version vs `required_version`; config validity; write access to `.cupel/`; one finding per problem with `action.kind: configure | install_tool`; exit 3 if anything required is missing, else 0. Tool checks are added as tool producers land (P3+). `--fix --install` is wired but has nothing to install until P3.

ADRs: [021](adr/CUPEL_DECISION_021.md) (detection, generated `cupel.toml`, preview without `--write`), [022](adr/CUPEL_DECISION_022.md) (`doctor` checks, findings, exit 3).

*Tests first:* `init --dry-run` golden per fixture repo type (minimal Cargo, npm, pyproject, Go module fixtures under `cupel-fixtures/repos/detect/`); `init` without `--write` writes nothing (assert tree unchanged); `--force` semantics; `doctor` on a repo with bad `required_version` → exit 3.

#### P0 Definition of Done

- All P0.1–P0.8 steps complete.
- `cupel describe --all` emits one JSON document covering verbs, flags, exit codes, schemas.
- Every verb from §6.2 is reachable; unimplemented ones return exit 2 with `cli.not-implemented`.
- `make check` green on Linux and macOS CI.
- README describes: purpose, install from source, `describe`, `init`, `doctor`, output contract, exit codes.

#### P0 Notes

- **P0 close-out.** All eight steps complete; 370 tests across `cupel-core` (165 + 1 doctest), `cupel-cli` (172), `cupel-fixtures` (24). `make check` is green locally on macOS arm64. The CI workflow exists but has not run: the branch has not been pushed, so the "Linux and macOS CI" DoD line is verified only for macOS until the first push. README rewritten to describe exactly what exists. Phase work was done by one agent per step (P0.3∥P0.4 and P0.5∥P0.6 in parallel worktrees); two ADR-number collisions were resolved at merge time (P0.3's CLI-contract ADR is 018, not 017), after which each agent was assigned reserved numbers up front.

- **P0.1 — lints.** `[workspace.lints]` sets `unsafe_code = "forbid"`, `clippy::all` and `clippy::pedantic` at `warn` (errors under `make lint`'s `-D warnings`), with three pedantic lints allowed workspace-wide: `module_name_repetitions` (domain names such as `fact::FactKind` repeat their module by design), `must_use_candidate` (fires on nearly every accessor and adds noise without catching bugs here), and `missing_errors_doc` (error variants are self-describing `thiserror` enums; per-function `# Errors` sections would repeat them). Any further allowance is per item with a justifying comment.
- **P0.1 — `clean-check`.** Implemented as a before/after fingerprint of the working tree rather than "porcelain must be empty", because the literal rule can never pass before a commit. See ADR-010.
- **P0.1 — Makefile targets.** `matrix` and `dogfood` are not defined yet; they are added in the steps that create the commands they run (P1.1 and P2.10). The `disallowed_methods` rule for `cupel-checks` is added with that crate in P2.1.
- **P0.1 — toolchain.** `rust-toolchain.toml` pins `1.92.0`; CI installs it with `rustup toolchain install` (rustup ≥ 1.28 reads the pin).
- **P0.2 — deviations from the step text, each with its ADR.** `LangId`/`LangFamily` live in `cupel-core` as open string newtypes, not in `cupel-lang` (ADR-011). `Evidence` is an open JSON object; per-kind evidence schemas live in `Manifest.findings[].evidence` (ADR-012). `SupportMatrix::compute` takes a fourth argument, the availability `Ctx`; resolution follows producer needs transitively; `Support` has a fourth variant, `Unconfigured(key)` (ADR-015). `schema::config()` does not exist yet — it lands with the config model in P0.4 — and `schema::plugin()` is the `Manifest` schema until P5.1 extends it (ADR-013).
- **P0.2 — staged context types.** The traits match design §8.3.3/§8.5, but their context types are deliberately minimal until the steps that give them content: `Scope { files }` (resolution in P2.1), `Ctx { root }`, `ProduceCtx { ctx }` (cancellation in P0.7, config in P0.4), `CheckCtx { config }` (store and scratch dir in P0.7/P2.2). `ProducerRegistry` stores producers type-erased behind `RegisteredProducer`, which exposes metadata and availability only; the erased `produce` call is added with the scheduler in P2.1.
- **P0.2 — open items for later steps.** P0.3 must define finding identity for findings without a symbol (CLI usage errors, `doctor`) before emitting `cli.not-implemented` (ADR-013 §8). `SymbolHash` validates `sh_` + lowercase hex of any length; P1.4 fixes the digest length when it first computes hashes. `run.started` is a string until P0.7 picks a time crate.
- **P0.2 — what the `FindingId` "ignores line numbers" test is.** The API has no parameter a line can enter through (`compute(check, kind, &SymbolHash, Option<&SiteHash>)`); a `compile_fail` doctest on `FindingId::compute` proves passing a `u32` line does not compile, and a proptest checks IDs are deterministic and well-formed over arbitrary structural inputs. Known-answer vectors were computed with the Python `blake3` package, independently of the Rust crate.
- **P0.2 — schemas.** Every property in every published schema is `required` (absent values are `null`, ADR-013), enforced by a schema transform and a test. Fourteen `insta` snapshots: `run`, `finding`, `events`, `plugin`, and one per syntactic fact kind. `CostClass` doubles as the depth type (`run.depth`); there is no separate `Depth` type.
- **P0.2 — tests.** 79 in `cupel-core`: `ids` 12 + 1 doctest, `contract` 20, `syntax_facts` 16, `extension` 22, `schemas` 8.
- **P0.3 — decisions.** [ADR-016](adr/CUPEL_DECISION_016.md): symbol-less findings are identified by `FindingId::for_subject(check, kind, subject)`, hashing an empty symbol slot so subject and symbol IDs never collide; CLI findings use `check: "cli"` and kinds `cli.usage`, `cli.not-implemented`, `cli.internal`; `doctor` (P0.8) should use `check: "doctor"` and the tool ID or config key as subject. [ADR-018](adr/CUPEL_DECISION_018.md): `describe` output lives in a new nullable `Envelope.description` (tagged `cli | command | schema | all`), `describe schema <name>` is an envelope carrying the schema, `run.verb` is nullable, unbuilt formats (`jsonl` P0.5, `sarif` P6.1) exit 2 rendered as JSON, exit codes are `stable` and everything else `experimental`, and `CUPEL_DEBUG_PANIC=1` is the deliberate exit-6 trigger.
- **P0.3 — core additions.** `FindingId::for_subject`, `ExitCode::meaning`, the `describe` payload types (`Description`, `CliDescription`, `CommandDescription`, `ArgDescription`, `ValueType`, …), `Envelope.description`, nullable `RunInfo.verb`, and `schema::{PUBLISHED, url, Generate}`. The `run` schema snapshot changed accordingly.
- **P0.3 — registry and clap.** Clap derive (`cupel-cli/src/cli.rs`) owns parsing and holds no human text; `registry.rs` owns every summary, value type, enum value, default, conflict, and `pending_step`, and applies `about`/`help` to the derived command before parsing. A unit test requires the clap tree and registry to agree argument by argument; `tests/help.rs` requires every `--help` page to mention exactly the registered flags and their summaries; `tests/exit_codes.rs` runs every registered example and checks its declared exit code. Defaults are described, never set on clap, so P0.4 can tell an unset flag from a default. No short flags exist (design §6.3 lists none).
- **P0.3 — merge points with P0.4.** `schema::PUBLISHED` in `cupel-core/src/schema.rs` is the one-line registration point for `("config", config)`; adding it puts `config` into `describe`, `describe schema`, and `describe --all` automatically, and the name-list assertions in `cupel-core/tests/description.rs` and `cupel-cli/tests/describe.rs` must then include it. `--config` and `--profile` already parse (`cli.global.config`, `cli.global.profile`) and carry `pending_step: P0.4` in the registry. `run.config` is `ConfigEcho::default()` in `cupel-cli/src/run.rs`. The repository root is `app::repository_root` (canonical `--cwd`, else the process working directory read once in `app::Environment`); config discovery should start there. `describe`'s `config_precedence` is a literal list in `registry::cli()`.
- **P0.3 — parsed but pending.** Flags whose behavior belongs to later steps parse, are echoed in `describe` with `pending_step`, and are otherwise ignored: `--dry-run`, `--events`, `--timeout` (accepted as a string until P0.7 parses durations), `--write` (P0.7); `--limit`, `--offset`, `--no-color` (P0.5); `--install`/`--no-install` (P3.1); every scope/depth/selection flag (P2.1–P5.4 as registered). `run.seed` echoes `--seed` and is 0 otherwise until P0.7 draws a random default. `run.started` is formatted by a dependency-free RFC 3339 formatter in `run.rs`, so P0.7 needs no time crate for it.
- **P0.3 — exit codes.** 0, 2, and 6 have deliberate triggers in `tests/exit_codes.rs`. 1, 3, 4, and 5 are described but unreachable until P2.9 (gate), P0.8 (`doctor`), P3.1 (baseline tests), and P0.7 (timeout); those steps add their triggers to that file.
- **P0.3 — deviations.** `cupel` with no arguments still prints usage to stderr and exits 2 (P0.1), and now also writes a `cli.usage` envelope to stdout; the P0.1 test was updated to say so. `--help` and `--version` print clap's human text to stdout and exit 0; `--describe` is the machine-readable form. The text format is a minimal stand-in in `cupel-cli/src/render.rs` (status line, description listing, findings with their action); P0.5 replaces that module without touching verb code. The workspace `clap` dependency gained the `env` feature for `CUPEL_FORMAT`.
- **P0.3 — tests.** 64 in `cupel-cli`: 20 unit (`registry` 3, `error` 4, `format` 5, `run` 3, `app` 5) and 44 integration (`cli_basics` 4, `describe` 14, `exit_codes` 13, `format` 12, `help` 1). `cupel-core` grew to 88 (`ids` +3, new `description` 6).
- **P0.4 — API for the CLI.** Fill `cupel_core::config::ConfigRequest { root, config_path, profile, flags: Settings, env }` (`root` is `--cwd`, absolute; `env` is the process environment — core reads neither cwd nor env itself) and call `Config::load(&request)`. The result has `effective: Effective { depth, format, budget, profile }`, the parsed `file: ConfigFile`, `path`, and `sources`; `Config::echo()` is the envelope's `run.config`. Every failure is `Error::Config(ConfigError::…)`, a usage error (exit 2). `Effective.format` is `None` when nothing set it: the CLI auto-detects from the terminal. `Format` (json/jsonl/text/sarif) and `TimeLimit` (`250ms`/`20s`/`5m`/`1h`) live in `cupel_core::config`; P0.3 should use them rather than CLI-local copies, and P0.7 should parse `--timeout` with `TimeLimit`.
- **P0.4 — what is layered.** Only `depth`, `format`, and `budget` are settings that flags, env, a profile, and the file can all set; env vars are `CUPEL_DEPTH`/`CUPEL_FORMAT`/`CUPEL_BUDGET` (defined once on `Setting`), and `CUPEL_PROFILE` selects a profile when `--profile` is absent. `sources` lists every contributing layer in precedence order. See ADR-017.
- **P0.4 — slices.** `Config::check_config(check, lang)` is what `CheckCtx.config` is filled from (per-language override deep-merged; the reserved `lang` key never reaches a check). `Config::lang_config(lang)` fills the new `ProduceCtx.config` field, so producers see `[lang.<id>]` only.
- **P0.4 — gate rules.** `[gate.changed]`/`[gate.tree]` parse into ordered `GateRuleSpec { key: GateKey { check, metric }, condition }` (toml's `preserve_order` feature keeps written order). Comparisons need a metric; `none`/`no-breaking` need a bare check ID. A malformed rule's error quotes the rule and points at its section header, not its line. Evaluation is P2.9.
- **P0.4 — registration point.** Parsing validates ID syntax only (check IDs are exactly `<group>.<name>`). `Config::validate_registered(&CheckRegistry, &[LangId])` rejects unknown checks, gate metrics not in the check's `Manifest.metrics`, and unknown languages; the CLI must call it once P2.1 (checks) and P1.1 (languages) populate the registries. `[checks."<id>"]` and `[lang.<id>]` values stay open tables until manifests declare parameters.
- **P0.4 — schemas.** `schema::config()` is the `cupel.toml` schema. Input schemas are generated without the ADR-013 "every property required" transform, so keys stay optional while closed sections keep `additionalProperties: false`.
- **P0.4 — tests.** 49 new in `cupel-core` (128 total): `config_gate` 5, `config_settings` 7, `config_file` 11 (three `insta` error-message snapshots), `config_discovery` 8 (`tempfile`), `config_resolve` 8 (16-row precedence matrix), `config_slices` 6, `config_registry` 2, `schemas` +2.
- **P0.5 — config wiring.** Every invocation that parses builds `ConfigRequest` (root from `--cwd`, `--config`, `--profile`, flags from `--format`/`--depth`/`--budget`, and the `CUPEL_*` variables collected once in `app::Environment.variables`) and calls `Config::load`; `run.config` is `Config::echo()`, `run.depth` the effective depth, and `run.budget_ms` the effective budget. The format is `Effective.format`, else the terminal rule. The CLI-local `Format` and `Depth` enums are gone: `--format`, `--depth`, and `--budget` parse into `config::Format`, `CostClass`, and `TimeLimit` through clap value parsers in `cli.rs`, so `cupel-core` stays free of clap. Clap no longer reads `CUPEL_FORMAT` itself (otherwise the env layer would be reported as `flags`); the registry still describes `env` on `--format`/`--depth`/`--budget`/`--profile`, and the registry unit test now checks that column against `Setting::env_var` instead of clap. Configuration errors map to `cli.usage` findings in `Usage::from_config` with ADR-016 subjects: `cupel.toml` (parse errors and unknown keys; the title names the key, the fix lists the valid keys, the detail carries line and column), `--config` (missing file), the variable name (`CUPEL_DEPTH`, …; an invalid `CUPEL_FORMAT` therefore names `CUPEL_FORMAT`, not `--format` as in P0.3), `--profile` (unknown profile; `valid` lists the defined ones), and the gate key. Errors raised before the format is known (bad `--cwd`, bad configuration) render in the format recovered from the raw arguments, as ADR-018 §5 does for parse errors. `--verbose` also prints the config path. `run.depth` is filled for every verb once configuration loads, so its doc comment (and the `run` schema snapshot) now say it is null only when configuration did not load; this fills, but does not change, the nullability of ADR-013 §3.
- **P0.5 — renderers.** `cupel-cli/src/render.rs` dispatches on `format::Rendering { Json, Jsonl, Text }` and holds `json` and `jsonl`; the text renderer is `render/text.rs` with the `describe` layouts in `render/text/description.rs`; SARIF (P6.1) is one more `Rendering` variant and function there. The JSONL header is the full envelope with `"findings": []`, so it parses as an `Envelope` (ADR-019 §1). Text prints every present value except evidence bodies (a `null`, an empty list, or a map of only absent values is not printed); findings are grouped by severity, then check in order of first appearance; `next_actions` is last. Color (ANSI SGR only) is on for text when stdout is a terminal, `--no-color` is absent, and `NO_COLOR` is unset or empty; the registry describes `NO_COLOR` as `--no-color`'s `env`. The renderer tests build their envelope in `render/fixture.rs` from struct literals, so a new envelope field fails to compile there until the fixture fills it.
- **P0.5 — paging.** `page::Page::apply` cuts `findings` after the verb and before rendering, for every format and every envelope of a parsed command line (including configuration and verb errors); `total_findings` counts before the cut and `truncated` is true exactly when findings were left out. A command line clap rejected is never paged, since its `--limit`/`--offset` cannot be trusted. `summary`, `checks`, `next_actions`, and the exit code are never changed by paging (ADR-019 §2).
- **P0.5 — changes to earlier tests.** `tests/format.rs`: the P0.3 text assertion was rewritten for the full layout, the not-implemented-format test now covers only `sarif`, and the invalid-`CUPEL_FORMAT` test expects the variable as the argument. `tests/exit_codes.rs::later_step_flags_parse_and_change_nothing` no longer passes `--config`/`--profile`. Text snapshots go through `support::redacted_text`, which masks the `run` line and the version.
- **P0.5 — open items.** The `describe` JSON and text snapshots assume no `cupel.toml` above `crates/cupel-cli` (they echo `run.config`); when the repository gains its own `cupel.toml` (P2.10), those tests should pass `--cwd` a temporary directory. The evidence summary points at `cupel explain <id>`, which arrives in P2.2. Text is not wrapped to the terminal width.
- **P0.5 — tests.** `cupel-cli` has 105 (40 unit, 65 integration), up from 64: `page` 8 (offset beyond the end, limit 0, exact boundary, overflow), `render` 6 (JSON, JSONL, and plain and colored text snapshots of the full fixture envelope, and a proptest that the JSONL header plus lines reassemble into the JSON envelope), `render::text::description` 5 (CLI and command snapshots), `app` +1 (color on a terminal, `--no-color`, `NO_COLOR`), `error` +1, `tests/config.rs` 12, `tests/render.rs` 9. `cupel-core` is unchanged at 128 plus 1 doctest.
- **P0.6 — API.** `cupel_fixtures::run_cupel(args, cwd) -> Outcome { code, stdout, stderr, envelope: Option<Envelope>, cwd }` runs the workspace-built binary (`assert_cmd`'s lookup) with stdin closed and `CUPEL_*` removed; JSON stdout that is not an envelope panics. `redact(&Envelope, root) -> Value`; `assert_golden(name, &Envelope, root)` and `Outcome::assert_golden(name)` (root = the run's `cwd`). `Goldens::new(dir, GoldenMode)` / `Goldens::from_env()` with `check(name, &Value) -> Result<(), GoldenError>` and `path(name)` is the testable core; `canonical_json` writes the file format. Golden names are slash-separated (`cli/describe`, later `<check>/<lang>`), so the §8.3.7 matrix test needs no harness changes. See [ADR-003](adr/CUPEL_DECISION_003.md).
- **P0.6 — decisions beyond the step text.** Redaction also masks `run.seed` (random by default from P0.7), `run.baseline.id`, `run.git.branch`/`dirty`, and the CLI version inside `describe`/`describe --all`; nulls are never masked. Goldens store keys sorted recursively and compare as parsed JSON. `similar` is pinned at 2.7 (insta's version) rather than 3.x. The CLI goldens run in a fresh temp directory so config discovery cannot see a repository `cupel.toml`; fixture repos must carry their own (ADR-003 Consequences).
- **P0.6 — staleness.** The harness uses whatever `cupel` binary is in the target directory. `make test` (`cargo test --workspace`) rebuilds it first; `cargo test -p cupel-fixtures` alone can run a stale binary. `UPDATE_GOLDENS` reaches the tests because `make` passes the environment through (verified by generating the first goldens with `UPDATE_GOLDENS=1 make test`).
- **P0.6 — layout.** Only `golden/cli/` exists; `repos/` and `conformance/` are created by the first step that puts a file in them (P0.8 `repos/detect/`, P1.1), since git does not track empty directories.
- **P0.6 — tests.** 21 in `cupel-fixtures`: `redact` 6, `golden` 10, `cli_goldens` 5 (three real goldens: `cli/describe`, `cli/describe-check`, `cli/unknown-verb`). Any change to `describe` output or the envelope (for example P0.5's config wiring populating `run.config`) changes these goldens; regenerate with `UPDATE_GOLDENS=1 make test` and review the diff.
- **P0.7 — API for later steps.** `cupel_core::Writer` is the only file writer: `Writer::new(root, write)` (root is canonicalized), `.recording()` for a dry run, `.confined()` (never outside `.cupel/`; what `ProduceCtx.writer` holds), `.scratch(&run)` (only `.cupel/work/<run>/`; what `CheckCtx.writer` holds), `write(path, bytes)`, `create_dir(path)`, `stream(path)` (an output channel such as `--events`, opened even while recording), `run_record(&envelope)`, `gitignore()`, `planned()`, and the layout paths `cupel_dir`/`cache_dir`/`work_dir(run)`/`run_record_path(run)`. Refusals and I/O failures are `Error::Write(WriteError::{Outside, Io})`. `RunPlan { would_run: PlannedCheck, would_skip: PlannedSkip (flattened `Skipped`), would_install: PlannedInstall, would_write: PlannedWrite { path, diff }, est_total_ms }` is `Envelope.plan`. `Cancellation` (`new`, `with_deadline`, `cancel`, `is_cancelled`, `checkpoint` → `Error::Cancelled`, `wait`) is `CheckCtx.cancel`/`ProduceCtx.cancel`. `JsonlSink<W: Write>` is the `EventSink` behind `--events`; `cupel_core::random_seed()` draws the default seed. See [ADR-020](adr/CUPEL_DECISION_020.md).
- **P0.7 — CLI wiring.** `app::accepted` resolves the root, builds the `Writer` (recording under `--dry-run`), opens `cupel-cli/src/events.rs::Events` (`stderr` or a root-relative path through `Writer::stream`; a refused path is a `cli.usage` finding on `--events`), loads configuration, runs the verb, emits `run.end`, and writes the run record. P0.8 `init --dry-run` gets `would_write` diffs by writing `cupel.toml` and `.cupel/.gitignore` through the writer it is given and returning `app::plan(run, RunPlan::default(), writer, events)`; P2.1 `check --dry-run` passes its `would_run`/`would_skip` the same way and emits its own events into `Events`. A run cut short is marked by `Run::interrupted` (status `partial`, exit 5, results kept; errors stay errors).
- **P0.7 — decisions beyond the step text.** The `Writer` writes `.cupel/.gitignore` (`*`, `!suppressions.toml`) itself whenever it creates `.cupel/`, so repositories that never ran `init` do not see run records in `git status`; `init` can still write it explicitly. `describe` and `--describe` keep no run record; every other parsed verb with a resolved root does, error envelopes included, as unpaged pretty JSON whatever `--format` says. A run record that cannot be written is a stderr line (even under `--quiet`), never a changed exit code. The events file obeys the `.cupel/` rule like any other write. `describe --dry-run` is a plan with empty lists (after validating the describe subject); `check --dry-run` and the other pending verbs stay `cli.not-implemented`. `run.started` keeps the hand-rolled RFC 3339 formatter in `run.rs`. `--seed` is described with default `random`. The exit-5 trigger is `CUPEL_DEBUG_SLEEP_MS=<ms>`: every verb but a dry run waits that long on the run's `Cancellation` before its own work.
- **P0.7 — changes to earlier tests.** `tests/support::run_with` runs every invocation in a fresh temporary directory, so pending verbs that now keep run records never write into `crates/cupel-cli`; `app.rs` unit tests likewise run in temporary directories. `support::redacted` masks `run.seed`, and `tests/render.rs` pins `--seed` where it compares two runs. `later_step_flags_parse_and_change_nothing` became `flags_that_do_not_apply_to_describe_change_nothing` (`--dry-run` and `--events` now change the output, so they left the list). The `describe` goldens and snapshots changed only where the five flags lost `pending_step: P0.7`, `--seed` gained `default: random`, `--events` gained a longer summary, and every envelope gained `"plan": null`; the `run` schema snapshot gained `plan` and the `Planned*`/`RunPlan` definitions.
- **P0.7 — open items.** Nothing deletes `.cupel/work/<run>/` or old run records; P3.4 (the first heavy scratch user) should add cleanup and P2.2 decides whether run records stay once the store holds runs. A dry run's own run record is not listed in `would_write` today because no built verb would write one; P2.1 adds `PlannedWrite { path: .cupel/runs/<id>.json, diff: null }` for `check --dry-run`. `doctor` (P0.8) should turn an unwritable `.cupel/` into a `configure` finding, since the run itself only reports it on stderr.
- **P0.7 — tests.** `cupel-core` has 162 plus 1 doctest (up from 128): `writer` 21 (including `..`, absolute, lookalike-sibling, symlink, dangling-symlink, and symlinked-`.cupel` escapes), `run_controls` 8, `contract` +2, `extension` +3. `cupel-cli` has 131 (48 unit, 83 integration), up from 105: `run_infrastructure` 17, `exit_codes` +1 (exit 5), `app` +5, `run` +3. `cupel-fixtures` is unchanged at 21.
- **P0.8 — decisions.** [ADR-021](adr/CUPEL_DECISION_021.md): detection rules and the default-command table, nesting decides workspace members, the generated `cupel.toml` shape (`[roots]` unless exactly one root at `.`; one `[lang.<id>]` table per language, taken from that language's first root; no `required_version`, no skip directories), `init` without `--write` is a preview (a `plan` envelope plus a `next_actions` entry `cupel init --write`, no run record), `--force`, and `default_branch` from git. [ADR-022](adr/CUPEL_DECISION_022.md): `doctor`'s findings (`doctor.config-invalid`, `doctor.version-unsatisfied`, `doctor.version-invalid`, `doctor.cupel-unwritable`, `doctor.tool-missing`, `doctor.fix-pending`), `semver` for `required_version`, any `fail` finding → `status: error`, exit 3.
- **P0.8 — detection lives in `cupel-cli`.** `cupel-core` may name no language (ADR-011) and `cupel-lang` does not exist yet, so the module is `cupel-cli/src/detect.rs` plus one file per ecosystem (`detect/{cargo,go,node,python,jvm}.rs`). The API is `detect::project(root) -> cupel_core::syntax::ProjectInfo`, the existing payload type, unchanged. Each ecosystem exports an `Ecosystem { manifests, skip_dirs, describe: fn(&Candidate) -> ProjectRoot }`; P1.9 turns each `describe` into its language's `ProjectInfo` producer and moves the command choices into the `cupel-adapt` toolchain table, after which `init` reads the fact instead of calling `detect::project`.
- **P0.8 — CLI wiring.** `Cli::plans_only()` (`--dry-run`, or `init` without `--write`) selects the recording writer, skips the run record and the debug slow task. An `init` preview plans through its own `Writer::new(root, true).recording()`, because a recording writer still refuses paths outside `.cupel/` without `--write` and the preview shows what `--write` would do. `app::runs_without_configuration` lets `init` and `doctor` run when `Config::load` fails: `execute` now takes `Result<&Config, &cupel_core::Error>`, `init` ignores the error, `doctor` reports it, and the envelope renders in the format recovered from the raw arguments. `cupel-cli/src/git.rs::default_branch` shells out to `git` until ADR-002 (P2.1) decides git access.
- **P0.8 — shared pieces.** `cupel-cli/src/problem.rs::Problem` builds every subject finding (ADR-016): `CupelError::finding` and `doctor` both use it. `Usage::from_core` (the `From<cupel_core::Error>` body) and `Usage::problem(check, kind)` let `doctor` reuse the exact subject, title, fix, and evidence a configuration error has as `cli.usage`. `Run::reported(status, exit_code, findings)` builds a verb's envelope with findings. `cupel_core::Writer::probe()` creates `.cupel/` if needed, then creates and removes `.cupel/.probe-<ulid>`; a recording writer touches nothing.
- **P0.8 — the toolchain seam.** `doctor::requirements(config)` is `git` (`git --version`, needed for `--changed`, not required) followed by `doctor::toolchain(config)`, which returns nothing until the P1.9 table and P3 producers exist. `doctor::tools(requirements, installed)` turns missing requirements into `doctor.tool-missing` findings (`fail` when required, else `warn`) and is unit-tested with fake requirements and a fake `installed`.
- **P0.8 — fixtures and goldens.** `repos/detect/{cargo-single,cargo-workspace,go-module,npm-js,pnpm-ts-workspace,python-pyproject,python-requirements,monorepo,maven,empty}` (`empty` holds only `.gitkeep`, since git tracks no empty directory) and `repos/doctor/required-version`. `cupel_fixtures::{repo_path, copy_repo}` locate and copy a fixture; golden runs happen in a temporary copy, outside any git repository and any `cupel.toml`, so `default_branch` is absent from the `init` goldens. Goldens: `golden/init/<case>.json` for all ten detect fixtures, `golden/doctor/empty.json` (exit 0), `golden/doctor/required-version.json` (exit 3). Doctor findings never repeat the binary version (it is `cupel.version`), so the goldens survive releases. The doctor goldens and the clean-findings assertion print `SKIPPED: git not installed` without git, because a missing git adds a warning.
- **P0.8 — changes to earlier tests.** The `Repo` helper moved from `tests/run_infrastructure.rs` to `tests/support` (gaining `write` and `run_with`) for the new `init`/`doctor` tests. `exit_2_for_every_unimplemented_verb_names_its_plan_step` expects 8 pending verbs, not 10. `tests/exit_codes.rs` gained the exit-3 trigger (`doctor` with `required_version = ">=99.0"`). The `describe` goldens and snapshots changed only in the `init` and `doctor` lines (summaries, `pending_step: null`); `describe init`/`describe doctor` list their real examples (all exit 0) and `--fix` carries `pending_step: P3.1`.
- **P0.8 — open items.** Generated configurations name `java` and `clojure` roots; when P1.1 calls `Config::validate_registered`, those IDs must be registered or `init` must omit such roots (ADR-021 Consequences). Two roots of one language share one `[lang.<id>]` table; per-root commands need a design change. Only `doctor` checks `required_version`. `doctor --dry-run` is an empty plan. Detection walks the whole tree below the root (skipping hidden and generated directories) with no depth limit; large repositories may want the `ignore` crate once P1.1 adds it for the file walk.
- **P0.8 — tests.** `cupel-cli` has 172 (67 unit, 105 integration), up from 131: `detect` 6 (one table-driven test over all ten fixtures, `go.work`, Gradle and `deps.edn`, two ecosystems in one directory with skipped generated directories, yarn and `node:test`, `mentions`), `detect::go` 1, `init` 5 (every generated file loads with `Config::load`, the exact monorepo file, name collisions and quoting, a single root below the top, TOML escaping), `git` 2, `doctor` 5 (fake requirements, probes, Cargo requirement syntax), `tests/init.rs` 9, `tests/doctor.rs` 12, `exit_codes` +1. `cupel-core` has 165 plus 1 doctest (`writer` +3 for `probe`). `cupel-fixtures` has 24 (`init_goldens` 1 over ten goldens, `doctor_goldens` 2).

---

### P1 — Language layer

**Goal:** `cupel-lang` with the `Language` trait (design §8.4.1), the generic query-driven producers, and the per-language `langs/<id>/` specs for JavaScript, TypeScript, Rust, Go, and Python — producing every syntactic fact in design §8.3.2; structural hashing; a content-hash parse cache; a parallel tree walk; and the per-fact conformance suites that make each producer an enforceable contract.

**Status:** complete (2026-10-13, fc06317)

Each step below delivers **one fact kind** through a generic producer plus any hand-written overrides, for all five languages, before moving on (§2.8). Fixture repos under `cupel-fixtures/repos/{javascript,typescript,rust,go,python}/` are created in P1.1 and grow with each step; every step adds `cupel-fixtures/conformance/<fact>.toml` and maps its constructs in each language's `inventory.toml`. By the end of P1 the support matrix for the P2 checks is fully `supported` for all five languages, and that is verified by `describe matrix`.

#### P1.1 Language trait, spec, generic-producer scaffold, grammars, walk, cache, conformance harness

**Status:** complete (2026-10-10, d78da11) · [ADR-023](adr/CUPEL_DECISION_023.md)

- `Language` trait exactly as design §8.4.1; `LangId`, `LangFamily`; language registry by extension; `grammar(path)` selecting among a language's grammars (`javascript`: js/jsx; `typescript`: typescript/tsx); `register()` attaching producers to the `ProducerRegistry` from P0.2.
- `LanguageSpec` loader: `queries/*.scm` embedded with `include_str!`, `tables.toml` parsed at startup once; a test compiles every `.scm` against its grammar and checks that every capture name a generic producer expects is present.
- Generic-producer scaffold: the shared machinery for walking captures over a parsed file (`QueryProducer<F>`), with per-fact producers added in P1.2–P1.8.
- `langs/<id>/` for all five languages with `mod.rs`, empty `queries/`, `tables.toml`; `javascript` and `typescript` share one implementation module with two `LangId`s and the `ecma` family.
- Parallel walk with `ignore` (gitignore-aware), rayon parse; parse cache keyed by blake3 of file content.
- Conformance harness: iterates `(fact kind, language)` × constructs from `conformance/<fact>.toml`, requires every construct mapped in `inventory.toml` (or `not-applicable` with reason), and runs the producer to assert detection.
- `cupel describe language <id>`, `describe languages`, `describe fact <kind>`, `describe matrix` render from the registries (the matrix is all-`unsupported` at this point and that is the correct output).

*Tests first:* every `.scm` compiles with the expected captures; walk returns exactly the fixture's expected file list (gitignored files excluded); cache hit rate is 100% on second walk; parse errors produce a `lang.parse-error` finding, never a panic; `.tsx` and `.jsx` files parse with the right grammar; conformance harness fails on a deliberately unmapped construct; `describe matrix` snapshot.

#### P1.2 Fact: `Symbols`

**Status:** complete (2026-10-10, c463b6a)

- `QuerySymbols` generic producer over `symbols.scm` → functions, methods, nested functions, closures assigned to names, with namespace (`module::Type::method`), visibility, span, body ref, `lang`. Language specifics expressed in queries/tables where possible: Go method receivers and package-level namespace; Rust `impl` blocks and trait impls; Python nested `def` and class methods; JS/TS arrow functions assigned to `const`, class methods, exported function expressions.

*Tests first:* expected-inventory JSON per fixture file (name, ns, visibility, start/end line); `conformance/symbols.toml` constructs mapped for all five languages.

#### P1.3 Fact: `Decisions`

**Status:** complete (2026-10-10, c463b6a) · [ADR-024](adr/CUPEL_DECISION_024.md)

- `QueryDecisions` over `decisions.scm` → `Decision { kind, nesting }` per symbol with the semantic kind enum from design §8.3.2; cyclomatic and cognitive weights per kind in `tables.toml`, following crapper's per-language tables (design §3). The producer emits decisions; *computing* complexity from them is the check's job (P2.1).

*Tests first:* functions with known decision lists; `conformance/decisions.toml` constructs (`if`, `loop`, `boolean-op`, `early-return`, `case`, `ternary`, `catch`) mapped for all five languages with `not-applicable` for e.g. Go ternary, Rust `switch`.

#### P1.4 Fact: `NormTree` and structural hash

**Status:** complete (2026-10-10, c463b6a) · [ADR-025](adr/CUPEL_DECISION_025.md)

- `TableNormalize` over normalize rules in `tables.toml` → `NormTree`: locals, fields, literals, keywords → generic markers; call names, operators, tree shape preserved (dryer's rules, design §3). Hand-written overrides only for constructs the rules cannot express (Python `self`, Rust macros, Go receivers), each justified by a comment and still passing the same inventory.
- `NormTree` serialization is language-independent so `ecma`-family symbols compare across JS and TS.
- `Symbol.hash = blake3(lang ∥ serialize(NormTree))`, `sh_` prefix.

*Tests first:* property tests — hash invariant under: rename of locals/params, reformatting (whitespace/newlines), function rename, file move; hash **changes** under: operator swap, added branch, changed call target. Fixture pairs for each language. A JS function and its TS-annotated twin normalize to equal `NormTree`s.

#### P1.5 Fact: `MutationSites`

**Status:** complete (2026-10-10, c463b6a) · [ADR-026](adr/CUPEL_DECISION_026.md)

- `QueryMutationSites` over `mutations.scm` → `Site { operator, set, from, to[], site_hash }` with the semantic operator enum; operator set `default` (boolean flips, relational/equality swaps, `+/-`, `*//`, `&&/||`, `0/1`) and `extended` (statement deletion, return-value replacement, constant replacement, boundary ±1, negation). `site_hash` is the structural position within the symbol, not the line. Replacement spellings live in `tables.toml` (Python `and/or`, Go/Rust/JS `&&/||`; Python `True/False`, others `true/false`).

*Tests first:* expected-sites JSON per fixture function; `site_hash` unchanged when the function is moved or reformatted (property); `conformance/mutation-sites.toml` mapped for all five; every generated replacement re-parses without a syntax error in its language (property over the fixture corpus).

#### P1.6 Facts: `TestFrames` and `TestConventions`

**Status:** complete (2026-10-10, c463b6a) · [ADR-027](adr/CUPEL_DECISION_027.md)

- `QueryTestFrames` over `tests.scm` → test cases with `Assertion { kind, strength }` (semantic kinds from design §8.3.2), mocks, skip markers, setup blocks, and the detected framework, for each language's dominant frameworks: vitest/jest/mocha/`node:test` (JS/TS), Rust `#[test]`/`#[tokio::test]`, Go `testing.T`/testify, pytest/unittest. The idiom→kind mapping (e.g. `assert x is not None` → `NotNull`, `expect(x).toBeTruthy()` → `Truthiness, Weak`) lives in `tables.toml`.
- `GlobTestConventions` over test-file globs in `tables.toml` → `TestConventions` (`is_test(path)`, `test_files_for(source)`): `*.test.ts`/`__tests__/`, `#[cfg(test)]`/`tests/`, `*_test.go`, `test_*.py`/`tests/`.

*Tests first:* expected-frames JSON per fixture test file; `conformance/test-frames.toml` mapped for all five; `test_files_for(src)` returns the fixture's actual test file for every source file that has one.

#### P1.7 Fact: `ModuleGraph`

**Status:** complete (2026-10-10, c463b6a) · [ADR-028](adr/CUPEL_DECISION_028.md)

- `QueryImports` over `imports.scm` → raw edges with specifiers (ESM/CJS `import`/`require`, Rust `use`/`mod`, Go `import`, Python `import`/`from … import`).
- Hand-written per-language resolution producing `ModuleGraph` with edges `File(path) | External(name) | Unresolved` and a `relative` flag: Node resolution with `package.json` `exports`/`main` and `tsconfig` `paths`; Rust `mod` tree and workspace members; Go module path → directory via `go.mod`; Python packages via `__init__.py` and project roots. This is the one syntactic fact that is mostly Rust rather than queries, by design.

*Tests first:* fixture per language with a known import graph (including one external and one unresolvable specifier); resolved graph equals the expected edge list exactly; a TS fixture with `paths` aliases resolves correctly; `conformance/module-graph.toml` mapped for all five.

#### P1.8 Facts: `ApiSurface` and `Roots`

**Status:** complete (2026-10-10, c463b6a) · [ADR-029](adr/CUPEL_DECISION_029.md)

- `QueryApi` over `api.scm` → exported/`pub`/capitalized/`__all__` items with normalized signatures.
- `QueryRoots` over roots rules in `tables.toml` plus `api.scm` → symbols live by convention: `main`, exported items, test functions, framework hooks (`init()`, `__init__`, default exports, `#[no_mangle]`, Go `TestMain`).

*Tests first:* expected API JSON per fixture; a fixture change that removes a public parameter shows as a difference between two `ApiSurface` facts; every test function in every fixture appears in `Roots`; `conformance/api-surface.toml` and `conformance/roots.toml` mapped for all five.

#### P1.9 Fact: `ProjectInfo`; toolchain table

**Status:** complete (2026-10-13, fc06317) · [ADR-036](adr/CUPEL_DECISION_036.md) · [ADR-037](adr/CUPEL_DECISION_037.md) · [ADR-038](adr/CUPEL_DECISION_038.md)

- Move P0.8's detection into each language's hand-written `ProjectInfo` producer (build file, roots, default test/coverage/bench commands, dirs to skip). JS vs TS decided by `tsconfig.json` presence.
- `cupel-adapt` toolchain table keyed by `LangId` per design §8.7: per-language rows naming runner, coverage, and bench producers with detection rules and default order; property/fuzz/race tool names. The producer implementations land in P3 — in P1 the table holds their IDs and detection rules so `init`, `doctor`, and `describe language` can already read it.

*Tests first:* P0.8's `init` goldens continue to pass unchanged; a monorepo fixture with a Rust crate, a Go module, a TS package, and a Python package yields four roots with the right languages; `describe language go` snapshot.

#### P1 Definition of Done

- Every syntactic fact in design §8.3.2 has a registered producer for all five languages, and every `conformance/<fact>.toml` is green with zero unmapped constructs.
- Each language's `mod.rs` is thin: every hand-written producer or override is justified by a comment naming the construct queries could not express.
- `describe fact <kind>` lists five producers for every syntactic fact kind. (`describe matrix` has no rows yet — checks arrive in P2 — and P2.1's matrix test is what proves the P1 facts suffice.)
- Parse of a 100k-LOC synthetic tree completes in < 2 s cold on the dev machine (record the number in Notes).
- README lists supported languages and the facts each produces.

#### P1 Notes

- **P1 close-out.** All nine steps complete; 655 tests (core 178, lang 238, cli 184, fixtures 38, adapt 17); `make check` green locally on macOS; CI still unverified because the branch has not been pushed. P1.1 and Rust were built by one agent as the reference; JS/TS, Go, and Python followed as three parallel worktree agents whose only shared-code changes were additive (`@export.name` capture, `[api] tight_after`, receiver-scoped API paths), merged without conflict beyond goldens and these notes. P1.9 was interrupted by an API rate limit mid-step and finished by a second agent from the partial tree. Convention recorded here: fixture source files under `cupel-fixtures/repos/` are analysis data, not Cupel code, and carry no `ABOUTME` headers. Known: `doctor` on the Cupel repo itself detects the Python/Go/JS fixture repos as roots until P2.10 writes Cupel's own `cupel.toml` with `[roots]` limited to the Rust crates.
- **P1.1 — crates and ABI.** `tree-sitter 0.27.1` reads grammar ABI 13–15; `tree-sitter-rust 0.24.2`, `-go 0.25.0`, `-python 0.25.0`, `-javascript 0.25.0` are ABI 15 and `tree-sitter-typescript 0.23.2` (typescript and tsx) is ABI 14, so the newest library works with all five. Also `ignore 0.4.33`, `globset 0.4.20` (used by `cupel-lang` and, for `TestConventions::is_test`, by `cupel-core`), `rayon 1.12.0`; all declared once in `[workspace.dependencies]`. Query cursors use tree-sitter's re-exported `StreamingIterator`.
- **P1.1 — layout.** `crates/cupel-lang/src/langs/{ecma,rust,go,python}/` each hold `mod.rs`, `queries/*.scm`, and `tables.toml`. `ecma` implements both `javascript` (`.js .jsx .mjs .cjs`, JavaScript grammar, which parses JSX) and `typescript` (`.ts .mts .cts` with the TypeScript grammar, `.tsx` with TSX). Shared ECMAScript queries live in `langs/ecma/queries/`; TypeScript-only patterns go in `langs/ecma/queries/typescript/<file>.scm` and are placed *before* the shared ones, so they win (ADR-023). Every `.scm` must be added to its language's `include_str!` list (`SHARED`/`TYPESCRIPT_ONLY` in `ecma/mod.rs`); `tests/spec.rs` fails on a query file that is on disk but not embedded, on a query that does not compile against every grammar of its language, and on capture names outside the contract.
- **P1.1 — the capture-name contract** is `cupel_lang::contract(file)` with the names as constants in `cupel_lang::capture` (`crates/cupel-lang/src/contract.rs`), summarized in the crate docs (`lib.rs`). What each capture means is documented on the producer that reads it: `cupel_lang::symbols`, `decisions`, `mutations`, `testframes`, `imports`, `api`, `roots`. Captures starting with `_` are free for predicates.
- **P1.1 — `tables.toml` reference** is `crates/cupel-lang/src/tables.rs`: `[namespace]` (`separator`, `source_roots`, `index_files`), `[visibility]` (`default`, `modifiers`, `modifier_default`, `[[visibility.names]]` with `prefix`/`suffix`/`capitalized`), `[decisions]` (`chain_parents`, `condition_fields`, `[decisions.weights.<kind>]`), `[normalize]` (`identifiers`, `literals`, `erase`, `flatten`, `operators`, `receivers`, `[[normalize.calls]]`), `[mutations.<operator>]` (`replace`, `templates`), `[tests]` (`framework`, `unknown_assertion`, `[tests.assertions]`), `[conventions]` (`test_globs`, `[[conventions.tests_for]]` with `template`/`stems`/`contains`), `[roots]` (`entry_points`, `hooks`). Each type's doc comment has a TOML example; unknown keys are errors. `langs/rust/tables.toml` is the complete example.
- **P1.1 — what registers a producer** (`crates/cupel-lang/src/generic.rs`): `symbols.scm` → `Symbols`; plus `decisions.scm` → `Decisions`; plus `[normalize]` → `NormTree`; plus `mutations.scm` → `MutationSites`; `tests.scm` → `TestFrames`; `[conventions]` → `TestConventions`; `api.scm` → `ApiSurface`; `symbols.scm` plus `[roots]` → `Roots`. `ModuleGraph` is hand-written: a language's `register()` adds `ImportGraph::new(language, cache, resolve)` with its own `resolve(&Imports) -> ModuleGraph` (see `langs/rust/mod.rs`). Every producer a language registers has ID `lang.<id>`, cost `quick`, no needs.
- **P1.1 — core additions.** `RegisteredProducer::produce` returns `Produced` (downcast to the fact, or `to_json`), which P2.1's scheduler can use as is; `Description` gains `language`, `languages`, `fact`, `matrix`; `schema::fact_version(kind)`. See ADR-023.
- **P1.1 — walk, parse, cache.** `cupel_lang::walk(root, &registry)` returns the registered languages' files, sorted, `/`-separated, honoring `.gitignore` with or without a git repository and skipping hidden entries; it does not yet honor `ProjectInfo.skip_dirs` (P1.9). `LanguageRegistry::builtin()` owns one in-memory `ParseCache` keyed by (BLAKE3 of the content, grammar), so identical files share a tree; the store replaces it in P2.2. `Parsed { file, lang, grammar, syntax }` is every extractor's input. Columns in every span are 1-based byte columns and `end_line`/`end_col` point just past the last byte.
- **P1.1 — parse errors.** A file that cannot be read, is not UTF-8, or parses with errors yields a `lang.parse-error` finding (check `lang`, subject the file, `warn`, location of the first `ERROR`/`MISSING` node) from every producer that parses it; files with errors are still extracted. P2.1 must de-duplicate these by finding ID. A broken fixture file fails conformance.
- **P1.1 — conformance.** The inventory format and detection rule are the `cupel_fixtures::conformance` module docs (ADR-023): constructs in `conformance/<fact>.toml`; mappings in `repos/<lang>/inventory.toml` as `[<fact>.<construct>]` with `file`, `name`, optional `expect` and `twin`, or `not-applicable = "<reason>"`. Only facts the language has a producer for are checked, and then every construct is required. Run one language alone with `CONFORMANCE_LANG=go cargo test -p cupel-fixtures --test conformance` (it calls the library, so no binary rebuild is needed). Every produced (language, fact) also has a golden of its full output over the fixture, `golden/facts/<lang>/<fact>.json`; `UPDATE_GOLDENS=1 make test` writes them, and each diff must be reviewed by hand. Inventories exist for `symbols`, `decisions`, `norm-tree`, `mutation-sites`, `test-frames`, `test-conventions`, `module-graph`, `api-surface`, `roots`.
- **P1.1 — `describe` and `make matrix`.** `describe language <id>|languages|fact <kind>|matrix` render from the registries (`cupel-cli/src/catalog.rs`); the matrix has no rows until P2.1 registers checks. Goldens: `golden/describe/{languages,language-<id>,fact-symbols,matrix}.json` and two text snapshots; agents adding producers in parallel will all touch `describe/languages.json` and their `language-<id>.json`, and resolve conflicts by regenerating.
- **P1.1 — open.** `Config::validate_registered` is still not called: with no checks registered it would reject every `[checks.*]` table, and `init` still writes `[lang.java]`/`[lang.clojure]` (ADR-021); P2.1 should call it once checks exist and decide those roots.
- **P1.2 — `Symbols`.** All five languages pass the `symbols` inventory with the queries written here; JavaScript, TypeScript, Go, and Python need review rather than new work (not yet covered: JS default exports and generator functions, TS abstract and overloaded methods, Go generic receivers beyond the patterns given, Python nested classes). When patterns overlap on one definition the earliest pattern in the file wins. Namespaces come from `[namespace]` path rules (or `@namespace.name`, Go's package clause), enclosing `@scope`s, and enclosing symbols, joined by the language separator; `@scope.name` inside a symbol's own match adds a non-lexical segment (Go receivers). `display` uses the language separator (`report::format::render`, `shapes.circle.Circle.area`).
- **P1.3 — `Decisions`** (ADR-024). Each symbol gets an entry (possibly empty). Nesting skips enclosing decisions reached through `condition_fields` and else-if chains via `chain_parents` (Rust and JS: `else_clause`; Go's else-if is an `if_statement` directly under its `if_statement`; Python's `elif_clause` sits under `if_statement`). Default weights are in code; a language overrides one kind with `[decisions.weights.<kind>]`. crapper's original tables were not available; the defaults are recorded in the ADR.
- **P1.4 — `NormTree` and hashes** (ADR-025). `SymbolHash` and `SiteHash` are now exactly 16 hex digits; the known-answer vectors in `cupel-core/tests/ids.rs` were recomputed with the Python `blake3` package. Hash = first 64 bits of `blake3(lang ∥ 0 ∥ json(root))`. The JavaScript/TypeScript `[normalize]` tables must make a JS function and its type-annotated TS twin normalize to equal trees (flatten `required_parameter`, erase `type_annotation` and friends); write that test with the tables. Property tests in `cupel-lang/tests/rust_norm.rs` are the pattern to copy.
- **P1.5 — `MutationSites`** (ADR-026). Sets follow the operator; replacements come from `[mutations.<operator>]`; `site_hash` hashes the operator and the child-index path from the symbol node. `cupel-lang/tests/rust_mutations.rs` has the re-parse property (every replacement parses) and the move/relayout property; copy both per language.
- **P1.6 — `TestFrames` and `TestConventions`** (ADR-027). Captures in a match that also captures `@test.case` belong to that case, which is how attributes and decorators outside a function attach; idioms that are not a callee name use `@assertion.idiom.<key>`; the file's framework comes from the earliest pattern with `@framework.<name>`. Conventions are path templates (`{base}tests/{stem}.rs`, `{dir}/{stem}_test.go`) with optional globs, stem filters, and a `contains` check.
- **P1.7 — `ModuleGraph`** (ADR-028). `imports.scm` yields raw imports (`cupel_lang::imports`); the resolver gets `Imports` (scope, file and directory checks, file text, and on-demand imports of any file through the cache). Roots are package directories found by the resolver (`.` for the repository root and for files outside any package) until `ProjectInfo` exists. The Rust resolver (`langs/rust/resolve.rs`) is the reference for the other three.
- **P1.8 — `ApiSurface` and `Roots`** (ADR-029). Signatures are the token span of the `@api.signature` captures with `@api.local` names as `_`; roots are one entry per (symbol, reason).
- **P1.2–P1.8 — what proved awkward in queries** (expect the same in other grammars): Rust attributes are *siblings* before the item they mark, so cases, skips, `#[should_panic]`, fixtures, and hooks use sibling-anchored patterns `((attribute_item …) . (attribute_item)* . (function_item) @…)` plus match-local ownership. Macro arguments are opaque token trees, so `assert!(x.is_some())` is recognized by an identifier inside the token tree. A function's `->` sits between `parameters` and `return_type`, which is why signatures are spans, not node texts. Grammars name the same thing differently (a class name is `identifier` in JavaScript and `type_identifier` in TypeScript; use `(_)`). Comments are tree-sitter extras but their inner tokens are not, so walks skip extra subtrees. Overlapping patterns are resolved by pattern order, so specific patterns go first. No generic producer needed a Rust-specific override: every Rust idiosyncrasy became a general mechanism (sibling ownership, idiom captures, `chain_parents`/`condition_fields`, `{}` templates, callee/receiver rules); the only hand-written Rust code is module resolution, by design.
- **P1.2–P1.8 — status.** Each step is `in progress` with Rust complete; `symbols` also conforms for the other four languages. A failing property test leaves a `*.proptest-regressions` file next to the test; delete stale ones rather than committing them.
- **Performance.** Cold walk plus parse of a synthetic 100,102-line tree (724 files, about 20,000 lines each of Rust, Go, Python, JavaScript, TypeScript, every file unique) takes 44–55 ms in a release build on an Apple M2 (8 cores); a warm re-parse (all cache hits) 8 ms; producing `Symbols` for all five languages over it 65–68 ms. The P1 DoD bar is 2 s.
- **Tests.** 461 in all: `cupel-core` 175 plus 1 doctest (`extension` +1 erased production, `ids` +2 and recomputed vectors, `syntax_facts` +1 `is_test`/`test_files_for`), `cupel-cli` 182 (`describe` +10, two text snapshots), `cupel-fixtures` 38 (`conformance` 8, `fact_goldens` 2, `describe_goldens` 4), `cupel-lang` 65 (`languages` 6, `spec` 4, `walk` 2, `cache` 2, `scaffold` 4, `rust_symbols` 6, `rust_decisions` 8, `rust_norm` 6, `rust_mutations` 6, `rust_tests` 6, `rust_modules` 4, `rust_api` 8, unit 3).
- **P1 — go — fixture.** `repos/go/` is module `example.com/ledger` (`go.mod` requiring uuid, testify, go-snaps, gomock) with a `go.work` whose second member is `shapes/` (`example.com/shapes`); `cmd/ledger/main.go` is the command, `report/` a subpackage, `ledger_test.go` standard-library tests, `pricing_test.go` testify tests. Every file is gofmt-clean; the root module does not build offline (its dependencies and the deliberately missing `example.com/ledger/gen` are not vendored). An editor's gopls may write `go.work.sum` into the fixture; it is not part of it.
- **P1 — go — no overrides.** Every Go fact except `ModuleGraph` is queries and `tables.toml`; `langs/go/resolve.rs` is the only hand-written code (ADR-033). Method receivers needed no `NormTree` override (ADR-032).
- **P1 — go — not-applicable.** `symbols`: `static-method` (every method has a receiver), `nested-function` (no named nested functions), `private` (unexported names are package-wide, which is `internal`). `decisions`: `ternary`, `catch` (errors are values tested by `if`). `test-conventions`: `inline-tests` (tests live only in `_test.go`). `module-graph`: `relative` (module mode has no relative imports; every edge has `relative = false`). `api-surface`: `module` (packages are directories, never declared). See ADR-032 and ADR-033 for visibility, test idioms, `nil` return-value mutants, subtests, and package imports as per-file edges.
- **P1 — go — generic change** (commit 3a22322, additive; Rust goldens unchanged): `api.rs` honors an `@scope.name` in an API item's own match as a non-lexical innermost path segment (as `symbols.rs` already did), and `tables.rs` gains `[api] tight_after` (default empty; tokens with no space after them in signatures), and the `_` local placeholder no longer counts as a name before `(`/`[`/`<`. ADR-033.
- **P1 — go — tests.** 45 new: `go_symbols` 5, `go_decisions` 8, `go_norm` 6, `go_mutations` 7, `go_tests` 6, `go_modules` 3, `go_api` 7, unit 3 (`langs::go::resolve` 2, `api` 1). `CONFORMANCE_LANG=go` is green for all nine facts; `golden/facts/go/*.json` and `golden/describe/{languages,language-go}.json` were regenerated.
- **P1 — python — coverage** ([ADR-034](adr/CUPEL_DECISION_034.md), [ADR-035](adr/CUPEL_DECISION_035.md)). Python produces all nine P1.2–P1.8 facts and conforms for every construct; two are `not-applicable`: `test-conventions.inline-tests` (Python runners collect tests from separate test modules; doctests are not modeled by `TestFrames`) and `api-surface.module` (Python declares no modules in source). The fixture is a src-layout `ledger` package (`pyproject.toml` with a uv workspace) plus the workspace member `libs/units`, with pytest (`tests/test_ledger.py`, `conftest.py`) and unittest (`tests/test_pricing.py`) tests.
- **P1 — python — the one override.** `langs/python/normalize.rs` post-processes the table-driven tree: a method's leading `self`/`cls` parameter and a body's leading docstring are dropped and the symbol rehashed (design §8.4.2's "implicit `self`"). Because `generic::register` would also register the table-driven `NormTree`, `langs/python/mod.rs` registers each generic producer itself with this one swapped in; `python_norm.rs` pins exactly one producer per syntactic fact (none yet for `ProjectInfo`) so a generic producer added later cannot silently skip Python. If Go needs the same for receivers, a generic `register` that accepts per-fact replacements would remove the hand-written list. Module resolution (`langs/python/resolve.rs`) is the other hand-written part, by design. No generic producer, table key, capture, or conformance file changed.
- **P1 — python — query techniques.** "Not listed in `__all__`" is expressed with earliest-pattern-wins: in symbols.scm a "listed → `@visibility.public`" pattern precedes "`__all__` present → `@visibility.internal`"; in api.scm a middle tier captures every remaining definition without `@api.name`, which hides it. `__all__` is read only as a list or tuple literal assigned before the definitions. Decorators are children of `decorated_definition` (not siblings, as Rust attributes are), so skip, mock, and fixture patterns match the `decorated_definition` and capture the inner `function_definition` as `@test.case`/`@test.setup`/`@root.hook`, which is the symbol's node. Grammar facts worth knowing: `is not` and `not in` are single anonymous tokens (with child tokens) usable as query literals and as `[normalize] operators`; `except*` is an `except_clause`; an f-string is a `string` with `interpolation` children; a predicate on an optional capture that did not match passes; `elif_clause` sits under `if_statement`, so `chain_parents = ["if_statement"]`.
- **P1 — python — observations for reconciliation.** (1) `cupel_lang::api`'s token joiner spaces the quote tokens of string literals (`= ""` reads `= " "`, `"m"` reads `" m "`) and puts a space in `[T] (Shape)`; output is stable, so comparison works, but any grammar whose strings have quote child tokens (JavaScript too) reads oddly; the fixture avoids a string forward-reference return annotation for that reason. (2) `Imports::is_file`/`is_dir` follow the file system, which is case-insensitive on macOS, so `ledger/Account.py` "exists" beside `account.py`; the Python resolver compares exact directory listings (cached per directory) instead, and other resolvers may want the same. (3) uv workspace member globs are expanded by a copy of the Rust resolver's member expansion (private there); a shared helper in `imports.rs` would remove it.
- **P1 — python — known limits** (ADR-034/035 consequences): a module-level definition inside `if`/`try` and a function nested in a compound statement of a function body take their visibility from their name; a `test*` function outside a test file is a case; a `Mock()` built inside a decorator argument counts as a module-level mock; every absolute import outside the repository is `External` by top-level module (typos included), since distribution names differ from module names; parameter names are `_` in signatures although keyword arguments make them part of a Python interface.
- **P1 — python — tests.** 515 in all (+54): `python_symbols` 5, `python_decisions` 10, `python_norm` 10 (including the rename/literal/indentation-relayout property and the one-producer-per-fact guard), `python_mutations` 8 (re-parse and move/relayout properties), `python_tests` 7, `python_modules` 6, `python_api` 8.
- **P1 — javascript/typescript — status.** All nine syntactic facts conform for both IDs (`CONFORMANCE_LANG=javascript|typescript`), with goldens under `golden/facts/{javascript,typescript}/`. One implementation (`langs/ecma/`) serves both; the only hand-written code is `resolve.rs` (module resolution), registered from `ecma/mod.rs`.
- **P1 — javascript/typescript — overlays.** TypeScript overlays: `symbols.scm` (accessibility modifiers; abstract classes and namespaces as scopes; `export =` names), `imports.scm` (`import x = require(…)`), `api.scm` (typed signatures, interfaces and their method signatures, type aliases, enums, namespaces, abstract classes, `export =`, private/protected exclusion). A JavaScript overlay was needed too: `queries/javascript/api.scm`, because tree-sitter rejects `(formal_parameters (identifier))` as an *impossible pattern* in the TypeScript grammars (every TypeScript parameter is wrapped in `required_parameter`/`optional_parameter`). `ecma/mod.rs` gains `JAVASCRIPT_ONLY`, placed before the shared queries like `TYPESCRIPT_ONLY`; `tests/spec.rs` lists `ecma/queries/javascript` so the embedding check covers it. Decisions, mutations, tests, and normalize needed no overlay: one shared file each.
- **P1 — javascript/typescript — decisions recorded.** ADR-030: a test callback passed to `it`/`test` after its title is a `closure` symbol named by the title, namespaced by `describe` suites, and is the `@test.case` node, so test cases own their decisions and sites and are `test` roots (ADR-029 had expected no JS test roots); anonymous default exports are functions named `default`; `exports.x = function` is a public closure. ADR-031: Node/TypeScript resolution subset, `node:<module>` for builtins, every non-workspace bare specifier `External`, and what counts as exported API.
- **P1 — javascript/typescript — generic change (one, additive, own commit `82f7f37`).** `symbols.scm` may capture `@export.name` (contract.rs, symbols.rs, crate-docs table): a top-level symbol named by such a capture, whose match fixes no visibility and has no modifier, is `public`. It covers `export { a }`, `export default a`, `module.exports = { a }`, `exports.x = a`, and `export = a`; Python's `__all__` has the same shape. No existing capture or table key changed meaning.
- **P1 — javascript/typescript — not-applicable.** `symbols.internal` for both (no package-level visibility: top-level names are exported or module-private; `#name`, `private`, and `protected` members are private) and `api-surface.interface` for JavaScript (no interfaces). Every other construct is exhibited, including `test-conventions.inline-tests` (vitest in-source tests, `import.meta.vitest`) and `roots.entry-point` (`[roots] entry_points = ["main"]`, for Node scripts that call `main()`).
- **P1 — javascript/typescript — tables.** `[normalize]` erases `type_annotation`, `type_parameters`, `type_arguments`, `decorator`, `accessibility_modifier`, `override_modifier` and flattens `required_parameter` and `assignment_pattern`, so a JS function and its annotated TS twin have equal `NormTree`s; their `SymbolHash`es still differ because the language is hashed (ADR-025), which the twin test asserts. Optional parameters (`x?`), non-null assertions (`x!`), and `as` casts are not erased. `[tests].framework = "jest"` (jest needs no import); frameworks come from imports (`vitest`, `@jest/globals`, `node:test`, `mocha`/`chai`) or `vi.`/`jest.` usage. Negated matchers and `assert.not*` are weak; `toThrow(expected)` and `assert.throws(f, expected)` are strong throws, bare `toThrow()` weak.
- **P1 — javascript/typescript — what was awkward.** Shared queries must compile against three grammars, and node kinds or parent–child pairs that exist in only one fail (`type_parameters` fields in JavaScript, bare-identifier parameters in TypeScript); keep shared patterns to common shapes and push the rest to overlays. Predicates on an absent optional capture pass, which is how `(accessibility_modifier)? @_a (#not-any-of? @_a "private" "protected")` excludes private members. When two patterns match one mock node, the match tree-sitter reports first wins regardless of pattern order, so keep mock patterns disjoint. A return that ends an inline callback (`items.forEach((x) => { …; return; })`) is an early return of the enclosing function unless the callback is itself in final position (ADR-024's structural rule).
- **P1 — javascript/typescript — tests.** 57 new: `cupel-lang` `ecma_symbols` 10, `ecma_decisions` 8, `ecma_norm` 8 (one property), `ecma_mutations` 8 (one property, re-parsing `.js`, `.ts`, and `.tsx` with their own grammars), `ecma_tests` 8, `ecma_modules` 4, `ecma_api` 7, and 4 unit tests in `langs/ecma/resolve.rs`. Workspace total 518.
- **P1.9 — the toolchain table** ([ADR-036](adr/CUPEL_DECISION_036.md)) is `crates/cupel-adapt/src/toolchain.toml`, embedded and parsed once; the format is documented in the `cupel-adapt` crate docs. One `[[rows]]` per language (`javascript` and `typescript` share one) lists `launchers` (package-manager prefixes whose `values` fill `<x>`, `<run>`, `<m>` placeholders), `runners`, `coverage` (with `format`), and `bench` in default order, each with `detect` rules (`{ file }`, `{ contains, in }`, `{ mentions, in = [globs] }`, `{ json, path, prefix?, members? }`, `{ all }`; any one selects, none means fallback) and `command` or per-runner `commands`; plus `property`, `fuzz`, `race` tool names and `[probes]` for programs without `--version` (`go version`). `toolchain::for_lang(id)?.select(&Root)` picks a root's tools and `Selection::commands()` fills `<…>` placeholders and `<packages>`; `{out}`, `{name}`, `{id}` stay for producers. The content is exactly P0.8's rules, so the `init` goldens did not move. `describe language <id>` shows the row (JSON `toolchain`; a `toolchain` text section).
- **P1.9 — dependency direction** ([ADR-037](adr/CUPEL_DECISION_037.md)). `cupel-adapt` depends on `cupel-core` only and `cupel-lang` does not depend on it. `ProjectInfo` is version 2 and reports detected facts only: `ProjectRoot { path, lang, build_files, members, packages, skip_dirs }` (`packages` is Go's `./...` or one `./<dir>/...` per `go.work` `use`; empty elsewhere); `DefaultCommands` is gone. The walk and nesting rule live in `cupel_lang::project`; each language's ecosystem is `langs/<dir>/project.rs` and its `mod.rs` registers `ProjectRoots` (ID `lang.<id>`, reads the repository root, not the scope). `cupel-cli/src/project.rs` runs every language's producer through the registry and joins a root with the table (`project::selection`/`commands`); `init` and `doctor` use it, and `cupel-cli`'s `detect` module is deleted. `cupel_lang::walk(root, languages, &ProjectInfo)` skips, below each root, the directories in its `skip_dirs` (closing the P1.1 open item). A `go.work` that uses `.` now covers `./...`; P0.8 produced `././...`.
- **P1.9 — Java and Clojure** ([ADR-038](adr/CUPEL_DECISION_038.md)). Maven, Gradle, and `deps.edn` roots are still detected (`cupel_lang::project::unregistered_roots`) but never written to `cupel.toml`; `init` reports each as an `info` finding `init.unsupported-root` (check `init`, subject `<lang>:<path>`, action `review`, evidence `{lang, path, build_files}`), titled "unsupported language root: java at ./svc — supported in a later release", in previews and after `--write`. The only `init` golden that changed is `init/maven.json`, which gains that finding (its planned `cupel.toml` is byte-identical); the other nine are byte-identical. The CLI now calls `Config::validate_languages` with the registered languages on every configuration load (unknown language: exit 2; `doctor` reports `doctor.config-invalid`; `init --force` replaces the file); P2.1 switches to `validate_registered`.
- **P1.9 — what `doctor` checks.** Besides `git`, the program of every command each language would use: configured `[lang.<id>] test`/`coverage`/`bench`, and for unset keys the table's default for the language's first detected root. Each `&&`-separated part's first word that is not `NAME=value` is probed with `<program> --version` (or `[probes]`); a missing one is `doctor.tool-missing`, `warn`, one per program, `needed_for` listing each use (`[lang.rust] test (tool.cargo-test)`; a configured command names only its key). None is `required` until P3, so tools never make `doctor` exit 3 yet; P3 replaces the probe with the producer's `availability`.
- **P1.9 — conformance and goldens.** `conformance/project-info.toml` has four constructs (`manifest-root`, `workspace-member`, `generated-dirs`, `packages`), mapped for all five languages (`packages` is `not-applicable` except for Go); `golden/facts/<lang>/project-info.json` exist for all five; `describe/language-<id>.json` and `languages.json` gained the `project-info` producer and the toolchain row; the `project-info` and run schema snapshots changed with the payload.
- **P1.9 — reconciliation items**, verified in code and tests: (1) one workspace-glob helper `Imports::expand` for Rust, Python, and JS/TS (`e3aaf35`); (2) exact-listing, case-sensitive module probes in every resolver (`986a3e3`); (3) string literals as single signature tokens (`3952745`); (4) ADR-029/030 status lines record the supersession (`5fa4f0b`); (5) Python registers through `generic::register_except` (`c6ff52d`); (6) `Config::validate_languages` for language-only validation (`bed0825`), now called by the CLI (`bf6312b`). ADRs 036–038 were committed with `bed0825`.
- **P1.9 — tests.** 655 in all (+33 since `bed0825`): `cupel-adapt` 17 (new: table loading and consistency, selection over every detect fixture, launchers, rules, describe), `cupel-lang` `project` 10 (including the four-root `monorepo` test) and `walk` +2, `cupel-cli` `describe` +2, `init` +1, `config` +2, `doctor` +1 and unit +3; `detect.rs`'s seven tests moved to `cupel-lang` and `cupel-adapt`. By crate: `cupel-core` 178, `cupel-lang` 238, `cupel-cli` 184, `cupel-fixtures` 38, `cupel-adapt` 17.

---

### P2 — Static checks (first vertical slices)

**Goal:** The `check` and `survey` verbs run real checks over a resolved scope, persist to the fact store, compare to baselines, honor suppressions, evaluate gates, and explain findings. Each check is a complete vertical slice.

**Status:** complete (2026-10-14, 24165fd)

Each check step (P2.3–P2.8) follows the same recipe and is not done until all of it is:

1. Fixture code exhibiting each finding kind (and clean code that must produce none) in all applicable languages.
2. Golden envelopes written **first** (hand-authored from the design's finding spec) and failing. The matrix test tells you exactly which `(check, lang)` goldens are required.
3. `Manifest` (id, version, tier, cost, `needs`, `wants`, flags, finding kinds → action kinds, metrics for gate keys, stability, docs). No `languages` field exists; applicability is derived.
4. `plan()` with cost estimate; `run(ctx, facts, plan, sink)` computing over `Facts` only and emitting findings through the `EventSink`. The module contains no language names, no `match lang`, no file or process access — the crate graph and clippy enforce this.
5. `describe <check-id>` and `describe finding <kind>` populated from the manifest; `describe matrix` row correct.
6. `explain <finding-id>` renders the check's evidence (source excerpt, pair side-by-side, etc.) — source excerpts come from `Symbols` spans, not from the check reading files.
7. Text renderer handles the new evidence shape in one line.
8. Goldens pass for every language the matrix marks `supported`; `make check` green; README check table updated.

#### P2.1 `Check` trait, registry, scope, scheduler v1, `check` verb with `static.complexity`

**Status:** complete (2026-10-13, 16df929) · [ADR-002](adr/CUPEL_DECISION_002.md) · [ADR-039](adr/CUPEL_DECISION_039.md) · [ADR-040](adr/CUPEL_DECISION_040.md) · [ADR-041](adr/CUPEL_DECISION_041.md) · [ADR-042](adr/CUPEL_DECISION_042.md) · [ADR-043](adr/CUPEL_DECISION_043.md)

The first end-to-end slice. Scope: make `cupel check --paths src/ --only static.complexity` produce correct `complexity.high`, `nesting.deep`, `params.many`, `function.long` findings with correct IDs, actions (`reduce_complexity`, `refactor_extract`), and repros — for all five languages, from one check module that names none of them.

- `Check` trait per design §8.5 with `run(ctx, facts, plan, sink)`; `CheckCtx`, `Plan`, `EventSink`; check registry by ID and group; `CheckCtx` merges `[checks."<id>".lang.<lang>]` overrides into the config slice the check sees.
- Scope resolution: `--paths` (globs, gitignore-aware), `--changed` (merge-base with `default_branch`, includes working tree), `--base/--head`, `--symbols` (ns::name or `sh_` hash); intersection semantics; empty `--changed` → `pass` envelope with `scope.symbols: 0`. Write **ADR-002** (git access).
- Check selection by depth (§5.8), `--only/--skip` (IDs and groups), `--tier N+`; per language in scope, drop `unsupported`/`unavailable` pairs from the support matrix and record each as `skipped` with reason.
- Scheduler v1 as a **fact DAG**: (check, lang) → needed facts → producers → their needs; shared facts produced once; sequential execution with rayon inside producers; no budget yet (P4.3).
- `static.complexity` as the first check: `needs = [Symbols, Decisions]`; cyclomatic and cognitive computed from `Decisions` using the weights the producer attached; thresholds configurable under `[checks."static.complexity"]` with per-language overrides.
- **The matrix test** lands here: enumerates registries, requires a golden per `supported` pair, a skip assertion per `unsupported` pair, and runs every check with only its declared facts.

*Tests first:* scope resolution matrix (unit, with a `tempfile` git repo); `--only`/`--skip` group expansion; golden envelopes for `static.complexity` on all five fixture languages; `scope.languages` lists exactly the languages that saw files; `--changed` on a clean tree → exit 0, `scope.symbols: 0`; a fact DAG unit test showing `Symbols` is produced once when two checks need it; a Go-specific threshold override changes only the Go findings; the matrix test itself fails when a golden is deleted.

#### P2.2 Fact store, `explain`, baselines, `suppress`

**Status:** complete (2026-10-13, 2068d40) · [ADR-045](adr/CUPEL_DECISION_045.md) · [ADR-046](adr/CUPEL_DECISION_046.md) · [ADR-047](adr/CUPEL_DECISION_047.md)

- `cupel-store`: SQLite WAL at `.cupel/cupel.db`; migrations; tables `symbols`, `metrics`, `runs`, `findings`, `baselines`, `suppressions` (remaining tables as their phases land). Single-writer lock.
- Every `check`/`survey` run persisted (envelope + findings + metrics).
- `explain <finding-id>` from the store; `explain <check-id>` returns last run's full per-symbol result. Never re-runs.
- `baseline set [--from run:ID] --write`, `show`, `diff [--against …]`, `export FILE`, `import FILE`, `--name`.
- `--against baseline|REF|run:ID`; `new_since_baseline` on findings; `summary.new/resolved`; `delta` on check results.
- `suppress <id> --reason --until REF|DATE` → `.cupel/suppressions.toml` (committable, so `.cupel/.gitignore` excludes it); suppressed findings appear with `suppressed: true`, `severity: info`; expired suppressions emit `suppression.expired`.

*Tests first:* store round-trip; migration from empty; `explain` on unknown ID → exit 2 envelope; baseline diff golden (run A, modify fixture, run B, `diff` shows exactly the new/resolved set); suppression lifecycle including expiry by date.

#### P2.3 `static.crap` (CC-only mode)

**Status:** complete (2026-10-14, cf7f341) · [ADR-048](adr/CUPEL_DECISION_048.md) · [ADR-049](adr/CUPEL_DECISION_049.md)

- `CC² × (1 − coverage)³ + CC` with `--no-coverage` forcing coverage = 0; without a `CoverageMap` producer (P3.2) the check reports `skipped: "no coverage; use --no-coverage"` rather than silently computing CC-only. Kinds `crap.high` (≥ 30), `crap.watch` (5–30).

#### P2.4 `static.clones`

**Status:** complete (2026-10-14, 2aa9540) · [ADR-050](adr/CUPEL_DECISION_050.md) · [ADR-051](adr/CUPEL_DECISION_051.md)

- Per-symbol subtree fingerprints from `NormTree`; MinHash signatures → LSH bands → pairwise Jaccard within buckets only. Defaults `threshold 0.82`, `min_lines 4`, `min_nodes 20`; same-family only (JS↔TS pairs are reported; Go↔Python never). Kinds `clone.pair` (both spans in evidence), `clone.cluster`.

*Additional tests:* property — MinHash/LSH never misses a pair whose true Jaccard ≥ threshold + 0.05 on generated fingerprint sets (recall bound); performance — 10k synthetic symbols in < 1 s.

#### P2.5 `static.deps`

**Status:** complete (2026-10-14, 5f378e4) · [ADR-052](adr/CUPEL_DECISION_052.md) · [ADR-053](adr/CUPEL_DECISION_053.md)

- Module graph from `imports()` + `resolve_import()` (P1.7); external and unresolved edges excluded from coupling but counted in evidence; afferent/efferent coupling, instability, abstractness, distance from main sequence; Tarjan SCC for cycles; layering rules from `[checks."static.deps".layers]`. Kinds `dep.cycle`, `dep.layer-violation`, `dep.unstable-abstract`.

#### P2.6 `tests.quality`

**Status:** complete (2026-10-14, f46a790) · [ADR-054](adr/CUPEL_DECISION_054.md) · [ADR-055](adr/CUPEL_DECISION_055.md)

- Over `test_frames()`: assertion density, weak assertions (`assert!(true)`, `assertTrue(x is not None)`-style), mock density, logic in tests, duplicated setup. Kinds per §5.2; each finding carries SCRAP-style remediation mode and actionability class in evidence.

#### P2.7 `tests.weakening`

**Status:** complete (2026-10-14, dd69c67) · [ADR-056](adr/CUPEL_DECISION_056.md) · [ADR-057](adr/CUPEL_DECISION_057.md)

- Diff of `test_frames()` between base and head: removed assertions, skipped/disabled tests, loosened matchers, rewritten snapshots, deleted tests. Default `severity: fail`. Requires a range scope; on a tree scope it is `skipped: "requires --changed or --base"`.

*Additional tests:* a fixture git history (constructed in a `tempfile` repo during the test) with one commit per weakening kind.

#### P2.8 `static.dead-code`

**Status:** complete (2026-10-14, c0981c0) · [ADR-058](adr/CUPEL_DECISION_058.md) · [ADR-059](adr/CUPEL_DECISION_059.md)

- Unreferenced symbols and exports using the symbol table, the resolved import graph, and `roots()` (P1.8) as the live set; conservative — anything reachable from a root or exported from a library root is not dead. Kinds `dead.symbol`, `dead.export`. `confidence` reflects uncertainty (dynamic dispatch, reflection, `getattr`).

#### P2.9 `survey`, gates, `next_actions`

**Status:** complete (2026-10-14, ab2b292) · [ADR-060](adr/CUPEL_DECISION_060.md) · [ADR-061](adr/CUPEL_DECISION_061.md) · [ADR-062](adr/CUPEL_DECISION_062.md) · [ADR-063](adr/CUPEL_DECISION_063.md)

- `survey`: same scope/depth; no gate; adds `rankings` (worst CRAP, largest clone clusters, most-coupled modules, dependency cycles; mutation/flaky dimensions join in P3).
- Gate evaluation per §6.7: `[gate.changed]` for range scopes, `[gate.tree]` otherwise; `--gate/--no-gate`; rule results in `summary.gate`; exit 1 on fail.
- `next_actions`: ranked by `severity × tier × new_since_baseline`, de-duplicated by action kind + location; `rerun` actions for skipped checks with their command.

*Tests first:* gate rule matrix (every operator, pass and fail); `survey` golden on each fixture; `next_actions[0]` is the highest-severity new finding in a constructed envelope.

#### P2.10 First dogfood

**Status:** complete (2026-10-14, 662b6f0) · [ADR-064](adr/CUPEL_DECISION_064.md) · [ADR-065](adr/CUPEL_DECISION_065.md) · [ADR-066](adr/CUPEL_DECISION_066.md) · [ADR-067](adr/CUPEL_DECISION_067.md) · [ADR-068](adr/CUPEL_DECISION_068.md) · [ADR-069](adr/CUPEL_DECISION_069.md) · [ADR-070](adr/CUPEL_DECISION_070.md) · [ADR-071](adr/CUPEL_DECISION_071.md) · [ADR-072](adr/CUPEL_DECISION_072.md)

- `cupel init --write` on the Cupel repo (commit the resulting `cupel.toml`); `[gate.changed]` with `tests.weakening = "none"` and `static.crap.max < 30` (CC-only until P3).
- `make dogfood` target; CI job runs it; `cupel check --depth quick` on the whole tree is clean or every finding is either fixed or suppressed with a reason.

#### P2 Definition of Done

- All seven static checks pass goldens in all five languages (or have a tested `skipped` reason from the support matrix).
- `cupel survey --depth quick` on the Cupel repo completes in < 5 s.
- `cupel check --changed --gate` passes in CI on the Cupel repo.
- `describe` covers every shipped check and finding kind.
- README: full check table, `survey`, `explain`, `baseline`, `suppress`, gate configuration, agent loop.

#### P2 Notes

- **P2.1 — shape of the pipeline.** `cupel-core` holds it all: `git` (ADR-002: `Git::open(root)`, `default_branch`, `head`, `commit`, `merge_base`, `changed_since`, `changed_between`, shelling out to the `git` on `PATH`; `cupel-cli/src/git.rs` is gone and `init` uses it), `scope` (`ScopeRequest { changed, base, head, paths, symbols }` → `resolve_files(tree, root, default_branch) -> ResolvedFiles { files, base }`, `mode()`, `selectors()`; `SymbolSelector`, `select_symbols`; `Scope { files, symbols: Option<BTreeSet<SymbolKey>> }` with `Scope::of_files` and `includes`), `select` (`Selection { depth, only, skip, tier }.select(&CheckRegistry)`), and `schedule` (`Scheduler { checks, producers }.run(&Job, sink)` / `.plan(&Job, sink)` → `Outcome { checks, findings, symbols, interrupted, plan }`, plus `next_actions(findings)`). Scope and selection errors are `Error::Argument(ArgumentError { argument, value, title, fix, valid })`, which the CLI maps to `cli.usage` with the flag as subject (ADR-040).
- **P2.1 — the erased fact path.** `RegisteredProducer::produce` already returned `Produced` (P1.1); P2.1 adds `Produced::share() -> SharedFact` (an `Arc<dyn Any + Send + Sync>` with its kind and JSON renderer) and `Facts::insert_shared`, so one production serves every check that declares the kind. `Facts` now holds `Arc`s; `Facts::get` of an undeclared kind is still `UndeclaredFact`. The scheduler's fact store keys productions by `(FactKind, LangId)`, picks the first `Ready` producer whose needs are producible, produces needs depth-first, and keeps each producer finding once by ID (the P1.1 `lang.parse-error` duplication). Producers do not yet receive upstream facts; `ProduceCtx` gains them with the first producer that has needs (P3.4).
- **P2.1 — contract changes.** `CheckCtx` gains `scope` (the files and symbols in scope for its language); `Check` gains a provided `combine(&[Score]) -> Option<Score>` (default `n`-weighted mean; `static.complexity` takes the maximum); `Plan::estimate(cost, targets)` and `CostClass::est_ms_per_target` (1 ms / 100 ms / 1 s / 10 s per target) stand in for history until the store; `ConfigError::CheckSetting { check, key, reason, valid }` is how a check rejects its settings; `Description` gains `Check(CheckDescription { manifest, languages })` and `Finding(FindingDescription)`. `Symbols` is version 2 with `SymbolDef.parameters` from new `@symbol.parameter` captures in every `symbols.scm` (ADR-039; a JavaScript `symbols.scm` overlay now exists for parameter forms the TypeScript grammars reject).
- **P2.1 — results.** One `checks[]` entry per check that ran (per-language results combined: `error` > `partial` > `fail` > `pass`; `fail` whenever it reported a `warn` or `fail` finding), then one `skipped` entry per (check, language) the matrix or the timeout ruled out, with `lang`, `fact`/`tool`, and `see: cupel describe matrix` (ADR-041). With no gate yet the envelope is `pass`/exit 0, or `partial`/exit 5 when `--timeout` cut work short; `summary.gate` is null and `new`/`resolved` are 0. `next_actions` are the top ten findings by severity then tier. `run.scope` is filled (mode, files, symbols, roots, languages) and `run.git` when in a work tree with a commit.
- **P2.1 — `cupel-checks`.** Depends on `cupel-core` (plus `serde`, `serde_json`, `schemars`); `crates/cupel-checks/clippy.toml` lists `disallowed-methods` (`std::process::Command::new`, `exit`, the `std::fs` readers and writers, `File::open/create`, `OpenOptions::open`, `std::env` variables, arguments, and `current_dir`) and `disallowed-types` (`Command`, `File`, `OpenOptions`); a deliberate violation was confirmed to fail `make lint`. `cupel_checks::register(&mut CheckRegistry)` is the CLI's one call (`catalog::checks()`). Private helpers: `finding::SymbolFinding` (ID, location, repro, suppress for a symbol finding) and `settings::Limits` (non-negative integer settings with defaults, unknown keys rejected). Adding a check: one module, one line in `register`, fixture code, goldens.
- **P2.1 — `static.complexity`** (ADR-042). Needs `Symbols`, `Decisions`, and `NormTree` — the step text named only the first two, but the finding ID and repro need the symbol hash, which only `NormTree` carries (ADR-014). Defaults `cyclomatic_max = 10`, `cognitive_max = 15`, `nesting_max = 4`, `params_max = 5`, `length_max = 60` (a finding fires above the limit); metrics (gate keys) `cyclomatic_max`, `cognitive_max`, `nesting_max`, `params_max`, `length_max` — single segments, since ADR-017's gate-key grammar allows no dotted metric such as `cyclomatic.max`. The nesting increment of cognitive complexity applies to the structural kinds (branch, loop, case, catch, ternary), because `Decision` does not carry `nests`. Score `{value: max cyclomatic, unit: cyclomatic_max, n: symbols}`. All findings are `warn`.
- **P2.1 — CLI.** `cupel-cli/src/check.rs` (`check::verb`): `project::detect` → one `cupel_lang::walk` → `ScopeRequest::resolve_files` → languages owning files → `Selection` → `Scheduler::run`/`plan`. `app.rs` changed in three places only: the `VerbArgs::Check` arm, `plan` became `pub(crate)` (the dry-run envelope builder), and `load_config` calls `Config::validate_registered(&catalog::checks(), &langs)` (full mode: unknown checks, gate metrics not in `Manifest.metrics`, unknown languages). `registry.rs`: `check` is no longer pending (examples `cupel check --only static.complexity` and `cupel check --depth quick --dry-run`, both exit 0) and `--changed`, `--base`, `--head`, `--paths`, `--symbols`, `--depth`, `--only`, `--skip`, `--tier` lost `pending_step`; `--since-baseline` and `--against` stay P2.2, `--budget` P4.3, `--gate`/`--no-gate` P2.9, `--oracle` P5.3, `--cases` P5.4. `describe <check-id>` and `describe finding <kind>` render from manifests (text layouts in `render/text/description.rs`); `describe matrix` has one row × five `supported` cells. A dry run's `would_write` lists `.cupel/runs/<id>.json` with a null diff.
- **P2.1 — seam for P2.2.** Findings and per-check results reach persistence only through the run record; `schedule.rs` carries the `// P2.2 persists this to the store` comment where they are assembled. Per-symbol metrics exist only in finding evidence and the score; P2.2 decides how the `metrics` table is filled (and P2.9 how gates read metric values).
- **P2.1 — the matrix test** (ADR-043) is `cupel_fixtures::matrix` (`Registries::builtin().pairs()`, `verify(&Goldens)`, `run_with_declared_facts(&Pair)`) driven by `tests/matrix.rs`; evidence is validated by `cupel_fixtures::evidence::validate`, a JSON Schema subset that refuses keywords it does not implement. Goldens: `golden/static.complexity/{go,javascript,python,rust,typescript}.json`, each with exactly one finding of each of the four kinds from the new `complexity` fixture file (`repos/<lang>/{src/complexity.rs, complexity.go, src/ledger/complexity.py, src/complexity.js, src/complexity.ts}`), whose remaining functions and every pre-existing fixture function stay within every limit. No pair is unsupported today, so the skip branch has no cases yet; the scheduler's skip recording is covered by `cupel-core/tests/scheduler.rs`.
- **P2.1 — changes to earlier tests and goldens.** The pending-verb stand-in in `tests/{format,render,run_infrastructure}.rs` and `app.rs` moved from `check` to `survey`; `exit_codes` expects 7 pending verbs; `describe` tests expect the check row, consumers, and support (the pending-step snapshot now describes `survey`); `go_modules` gained the `complexity.go` edges; every `golden/facts/<lang>/*.json` gained the complexity fixture's facts and `symbols.json` the `parameters` field; `describe/*` and `cli/describe*` goldens and the `run`, `symbols`, and CLI text snapshots were regenerated and reviewed.
- **P2.1 — tests.** 715 in all (+60): `cupel-core` 210 (`git` 7, `scope` 8, `selection` 5, `scheduler` 12), `cupel-checks` 9 (`static_complexity`), `cupel-cli` 194 (`check` 9, `describe` +3), `cupel-fixtures` 47 (`matrix` 3, `check_runs` 3, `evidence` 3), `cupel-lang` 238, `cupel-adapt` 17. The fact-DAG test registers two synthetic checks over a counting `Symbols` producer; the `--timeout 1ms` test runs the real binary on the Rust fixture.
- **P2.1 — open items.** Two structurally identical symbols share a hash and therefore a finding ID (both are reported). `--paths` and `--symbols` that match nothing are usage errors (a `--symbols` miss is allowed under a range). Range scopes analyze the working-tree versions of the changed files. Every `check` run spends roughly 60 ms in git for `run.git` (150 ms with `--changed`), measured in ADR-002.
- **P2.2 — the store API** (`crates/cupel-store`, depends on `cupel-core` only; [ADR-045](adr/CUPEL_DECISION_045.md)). `Store::open(&Writer)` creates `.cupel/` through `Writer::create_dir` and opens `.cupel/cupel.db` (WAL, `synchronous = NORMAL`, 5 s busy timeout), applying the embedded migrations in `migrations.rs` (`schema_version` holds the count; a newer store is `StoreError::TooNew`); it refuses a recording writer. `Store::open_existing(&Writer)` opens an existing store read-only as an SQLite `immutable` URI and creates nothing, for dry runs. Writes: `record_run(&Envelope)`, `record_metrics(run, check, &[Metric])`, `set_baseline(name, run)`, `put_fact(&StoredFact)`, `record_symbols(run, &[(SymbolHash, SymbolDef)])`, `replace_suppressions(&[Suppression])`. Reads: `run(id)`, `finding(id) -> Option<(Finding, RunId)>` (latest run that produced it), `last_run_for_check(check) -> Option<CheckRun { run, result, findings, metrics }>`, `last_result_run()`, `last_result_run_at(sha)`, `baseline(name)`, `fact(key)`, `symbols_new_since(run, &[SymbolHash])`, `suppressions()`, `schema_version()`. `CacheKey { kind, version, producer, inputs, toolchain_hash, config_hash }::digest()` is the §8.3.6 key (64 hex, inputs order-independent). All §8.8 tables exist; `coverage`, `mutants`, `test_results`, `oracles` are empty until P3/P5; `facts`, `symbols`, and per-symbol `metrics` have APIs but no writer until the scheduler uses them.
- **P2.2 — single writer.** Each write is one `BEGIN IMMEDIATE` transaction holding an exclusive `std::fs::File::try_lock` on `.cupel/cupel.lock`, retried for up to 5 s (`StoreError::Locked`); readers never lock. Eight threads recording concurrently is a test. The lock file stays; SQLite removes `-wal`/`-shm` on the last close, so after a run `.cupel/` holds `.gitignore`, `cupel.db`, `cupel.lock`, and `runs/`.
- **P2.2 — persistence hook.** `app::accepted` calls `persist::record` right after the run record, under `keeps_run_record` (not `describe`, not a dry run or preview). A failure is the stderr line `cupel: could not record the run in the fact store: …` and changes nothing else. It also mirrors `.cupel/suppressions.toml` into the store. Run records under `.cupel/runs/` stay (ADR-045 §9); nothing prunes either yet. Runs of `explain`, `baseline`, and `suppress` store only their envelope: their findings repeat earlier runs', and indexing them would make an `explain` "the latest run that produced" a finding.
- **P2.2 — for P2.1 and P2.9.** `cupel_core::deltas(current: &Envelope, against: &Envelope) -> Deltas { new: Vec<FindingId>, resolved: Vec<Finding>, scores: BTreeMap<CheckId, f64> }` and `Deltas::apply(&self, &mut Envelope, baseline: BaselineRef, vs: &str)` (sets `new_since_baseline`, `summary.new/resolved`, `run.baseline`, `checks[].delta`). `cupel_core::apply_suppressions(&mut Envelope, &[Suppression], expired: impl Fn(&Until) -> bool)`. In the CLI, `app::graded` already runs on every finished envelope: `suppress::apply` for `init`, `doctor`, `check`, `survey`, `tests`, then `against::compare(&mut envelope, writer, scope.against)` for `check` and `survey` (default: the `default` baseline when set, else nothing). P2.1's `check` arm therefore gets suppressions and deltas without calling anything, as long as it returns its envelope from `execute`'s match; the gate (P2.9) should evaluate after `graded`, so it sees suppressed severities and `new_since_baseline`. `against::resolve(store, root, spec, name)` is the reusable resolution. `--since-baseline` stays `pending_step: P2.1`: the store answers `symbols_new_since(baseline_run, hashes)` once the scheduler calls `record_symbols(run, …)` with the `Symbols` fact and `NormTree` hashes; scope resolution turns that into a scope.
- **P2.2 — `explain`.** A finding ID (`fnd_…`) gives the stored finding (full evidence, repro, suppress) plus `Envelope.explanation` (new nullable field per ADR-018 §1; `Explanation { subject, target, run, excerpt, covering_tests, nearest_test_file, metrics }`): a ±3-line excerpt of `location.file` (only inside the root), and `covering_tests`/`nearest_test_file` from `action.hints`. A check ID gives that check's last stored result in `checks`, its findings in that run, and its metrics. Unknown or malformed IDs exit 2 with argument `TARGET`. The text renderer prints an `explain` section with the excerpt (`>` marks the location's lines) and the explained finding's full evidence, the one place evidence bodies are printed. Every golden gained `"explanation": null`. Check findings will get richer excerpts (`Symbols` spans, clone pairs) as each P2.3+ check defines its evidence.
- **P2.2 — `baseline`** ([ADR-046](adr/CUPEL_DECISION_046.md)). `--name` belongs to the verb, so it comes before the sub-verb (`cupel baseline --name release show`); it is not global, which keeps the clap/registry agreement test as it is. `set` and `import` without `--write` are previews (`Cli::plans_only` now covers them): a plan whose `would_write` is `.cupel/cupel.db` with a diff of `baselines/<name>`, built by the new `PlannedWrite::change(path, label, before, after)` (the `Writer`'s own diffs now use it too). "Current" is the latest *result run* (`init`/`doctor`/`check`/`survey`/`tests`, exit not 2 or 6). A git ref resolves only to stored runs whose `run.git.sha` matches, and the run path does not fill `run.git` yet, so `--against <ref>` is always a usage error until P2.1 (ADR-002) records git info. Store errors in these verbs are `cli.usage` on `.cupel/cupel.db` (exit 2).
- **P2.2 — `suppress`** ([ADR-047](adr/CUPEL_DECISION_047.md)). `.cupel/suppressions.toml` is `[[suppression]]` tables (`finding`, `reason`, optional `until`, `created`) sorted by ID, read and written by `cupel_core::Suppressions`. `--until` is a date when shaped `NNNN-NN-NN` (it must then be a real date) and a ref otherwise; refs expire via `git::head_contains` (`rev-parse` then `merge-base --is-ancestor`). A malformed file is exit 2 with argument `.cupel/suppressions.toml`, for every verb that applies suppressions. An exit-3 envelope whose fail findings are all suppressed becomes `pass`/0; nothing else changes an exit code. `git.rs` gained `commit(root, ref)` and `head_contains(root, ref)`; it still shells out, pending ADR-002.
- **P2.2 — files P2.1 also touches.** `app.rs` (three verb arms, `graded`, the store write beside the run record, `plan` now `pub(crate)`), `cli.rs` (`plans_only`), `registry.rs` (`explain`/`baseline`/`suppress` and `--against` un-pended, `--since-baseline` now pending P2.1, three `baseline` summaries), `error.rs` (`CupelError::bad_argument`), `git.rs`, `main.rs`, `Cargo.toml` (member `cupel-store`, `rusqlite = 0.40.2` with `bundled`), `tests/exit_codes.rs` (5 pending verbs), `tests/init.rs` (`.cupel/` listing), `tests/support/mod.rs` (`Repo::store/seed/tree_outside_cupel`, `envelope`, `run_id`, `complexity_finding`, `complexity_check`), and every golden (`explanation: null`, pending markers).
- **P2.2 — tests.** 736 in all: `cupel-core` 194 (+16: `deltas` 4, `suppressions` 8, `writer` +2, `contract` +2), `cupel-store` 21 (new), `cupel-cli` 227 (+43: `explain` 10 + 2 unit, `baseline` 13, `suppress` 10, `fact_store` 5, `against` 3 unit), `cupel-fixtures` 39 (`baseline_goldens` 1: doctor run A with missing runner a, `baseline set --write`, runner changed to b, run B, `golden/baseline/diff.json` shows b new and a resolved), `cupel-lang` 238, `cupel-adapt` 17.
- **P2.3 — `static.crap`** ([ADR-048](adr/CUPEL_DECISION_048.md)). Needs `Symbols`, `Decisions`, `NormTree`; wants `CoverageMap` and `TestConventions`. CRAP = `CC² × (1 − coverage)³ + CC` with CC from `metrics::cyclomatic` (new private module, now also used by `static.complexity`). `CoverageMap` has no payload type yet, so coverage is never measured: with `[checks."static.crap"] no_coverage = true` coverage counts as 0; without it each language with symbols in scope is a `skipped` result `{reason: unconfigured, fact: coverage-map, lang, see: "cupel describe static.crap"}` (the closed `SkipReason` has no free text, so the plan's "no coverage; use --no-coverage" is carried by the fact and the `describe` summary). `--no-coverage` does not exist; it should set the same key. Settings `high = 30`, `watch = 5`, `no_coverage` (`settings::Limits::with_flags`/`flag` added for booleans). `crap.high` (≥ high, `warn`, `write_test`, `hints.nearest_test_file` from `TestConventions`), `crap.watch` (≥ watch and < high, `info`, `review`); evidence `crap`, `cyclomatic`, `coverage` (null), `watch`, `high`. Symbols in test files (per `TestConventions`) are not scored. Score `{value: max CRAP, unit: "max", n}`, combined by maximum; metrics `max`, `mean`; `fail` only with a `crap.high`.
- **P2.3 — for P3.2.** Give `CoverageMap` a payload type, compute per-symbol coverage over the symbol span, fill `coverage`, stop skipping when the fact is present, add `--no-coverage` writing `no_coverage`, and bump the check version.
- **P2.3 — outside the check** ([ADR-049](adr/CUPEL_DECISION_049.md)). `cupel-core/src/schedule.rs`: a per-language `skipped` result interrupts the run (exit 5) only when its reason is `timeout`; before, any check that skipped itself made the run `partial` (own commit 4564181, test in `cupel-core/tests/scheduler.rs`). Each language fixture repository has a `cupel.toml` with `no_coverage = true`, so every `static.complexity` golden now echoes it in `run.config` (and its `n`/`files` grew by the `crap_scores` file; its findings are unchanged). `cupel-cli/tests/check.rs` dry-run and events tests now pass `--only static.complexity`; `describe.rs` expects the `static.crap` row, consumer, and 10 matrix cells; the two describe text snapshots, `golden/describe/*`, and `golden/facts/<lang>/*` were regenerated; `go_modules.rs` gained the `crap_scores.go` edge.
- **P2.3 — fixtures and tests.** `repos/<lang>/{crap_scores.go, src/crap_scores.rs, src/ledger/crap_scores.py, src/crap_scores.js, src/crap_scores.ts}`: `overdue` (CC 6, CRAP 42, high), `clamp` (CC 3, CRAP 12, watch), `to_cents` (clean), all within every `static.complexity` limit. Goldens `golden/static.crap/<lang>.json` also carry the existing fixtures' bands (`label` 132, `settle` 42, the JS/TS `decide.classify` at exactly 30 with its test-file hint). 814 tests (+18): `cupel-checks` `static_crap` 13, `cupel-core` `scheduler` +1, `cupel-fixtures` `crap_runs` 4 (skip without the key, `describe` check and kinds, `explain` excerpt/evidence/test hint, one-line text evidence).
- **P2.3 — open items.** At coverage 0 every function with one branch is `crap.watch` (CC 2 → 6), so `info` findings are many. The `kind`/`metric` manifest helpers are duplicated between `static_complexity.rs` and `static_crap.rs` (left so concurrent P2 checks do not collide; worth one shared module at merge). No README check table exists yet to update.
- **P2.8 — what the check can see** ([ADR-058](adr/CUPEL_DECISION_058.md)). No fact carries references: `Symbols` has spans but no names used, `NormTree` labels identifiers `$id`, and `ModuleGraph` edges are file-to-file without the imported names. `static.dead-code` therefore decides reachability per *module* (files sharing a module namespace: the shortest namespace among a file's symbols, a method's without its receiver; the separator is derived from `display`), never per call. A module is live when an import from outside it resolves to one of its files, or when it holds an `entry-point`, `test`, or `framework-hook` root. An unreached module's exports are `dead.export`; an unreached module with no exports has every top-level symbol reported as `dead.symbol`. Blind spots: calls (a private helper in an imported file is never reported, e.g. the Rust fixture's `lib.rs::scale`), imported names, dead clusters importing each other, module-level code and scripts, implicit loading (Python `__init__.py`), and dynamic loading by computed name. A `References` fact is proposed in ADR-058 for P6.5/P9.
- **P2.8 — findings** ([ADR-059](adr/CUPEL_DECISION_059.md)). Needs `Symbols`, `NormTree` (finding identity, as ADR-042), `ModuleGraph`, `Roots`; `ApiSurface` is not declared (nothing it adds is used). `dead.symbol` is `warn`, confidence 0.8; `dead.export` is `info`, confidence 0.4, because no fact says whether a root publishes a library; both drop (0.2/0.1) when an `Unresolved`/`External` specifier has a segment equal to the file's stem. Action `review` with a summary naming the symbol and file; `detail` states what was and was not seen. Evidence `file`, `module`, `module_files`, `suspect_imports`. Metrics `dead_symbols`, `dead_exports`; score `{value: dead symbols, unit: dead_symbols, n}`, combined by sum; status `fail` only when a `dead.symbol` is reported. No settings.
- **P2.8 — open item: narrowed scopes.** Producers build `ModuleGraph` over the files in scope, so under `--paths`/`--changed` importers outside the scope are invisible and files read as unimported (a `--changed` run that edits one file reports its exports). `CheckCtx` does not carry the scope mode; `tests.weakening` (P2.7) needs it too, and one shared field (or the scheduler producing `ModuleGraph` over the whole walk) should land with whichever step first touches `cupel-core`.
- **P2.8 — fixtures and goldens.** Each fixture gains `dead_export` (one exported function) and `dead_helpers` (one private function), imported by nothing: `src/dead_*.rs`, `src/dead_*.{js,ts}`, `src/ledger/dead_*.py`, and Go packages `dead_export/` and `dead_helpers/` (a Go file in an imported package is live with its package). `golden/static.dead-code/<lang>.json` record those two plus, as `dead.export` (info), every export in a fixture file nothing imports: Rust `complexity.rs` (not in the `mod` tree); JavaScript `complexity`, `ledger`, `pricing`, `twins`; TypeScript `complexity`, `ledger`, `legacy`, `twins`; Python `twins` and `report/__init__.py`'s `summary` (Python `complexity.py` holds a dunder hook, so it is rooted). Each was checked against the fixture source: none is imported. Fixture files other steps add without importing them will add `dead.export` findings here. `static.complexity` goldens changed only in `scope.files`, `scope.symbols`, and `score.n`; fact goldens gained the new symbols.
- **P2.8 — changes outside the check.** `cupel-cli/tests/check.rs` now passes `--only static.complexity` where it asserts that check's findings, events, or plan; `cupel-cli/tests/describe.rs` and two `describe` snapshots list the second check. Every check added after this changes the same lists.
- **P2.8 — tests.** `cupel-checks` 23 (`static_dead_code` 14 over synthetic facts); 810 in the workspace.
- **P2.5 — `static.deps`** ([ADR-052](adr/CUPEL_DECISION_052.md), [ADR-053](adr/CUPEL_DECISION_053.md)). `needs = [ModuleGraph, ApiSurface]`: abstractness comes from `ApiSurface` (`interface` items over `type` + `interface` items), because `Symbols` holds only functions, methods, and closures. A module is a file in every language; Go package imports already fan out to one edge per package file (ADR-033), so a package cycle is a cycle between files. Self-edges (Rust `mod tests` → `super::*`) are dropped; Ca/Ce count distinct modules over `File` edges; external packages and unresolved imports appear only in `dep.unstable-abstract` evidence. D is defined only for modules with a coupling and a public type, and `dep.unstable-abstract` fires above the main sequence (A + I − 1 > `distance_max`, default 0.7). Settings `distance_max` and `layers` (`order`, `map`; `globset` with `literal_separator`, first matching layer wins, relaxed layering), validated at plan time. Findings have no symbol; IDs are `FindingId::for_subject` over the sorted cycle members, `from -> to`, or the module path; the repro is `cupel check --only static.deps`. Score `{value: cycles, unit: "cycles", n: modules}`, summed across languages; metrics `cycles`, `max_distance`, `violations`. `cupel-checks` now depends on `globset` (workspace dependency, not a crate-graph change).
- **P2.5 — fixtures.** Each language repository has `deps_cycle/` (two modules importing each other), `deps_layers/` (`domain` importing `infra`, and except in JavaScript an interface-only `port` that imports `domain` and that nothing imports), and a `cupel.toml` with only `[checks."static.deps".layers]` mapping `**/deps_layers/{domain,infra}/**`. That file is discovered by every run in the repository, so every check's golden for the five fixture repositories echoes it under `run.config` (the reason the `static.complexity` goldens changed; their findings did not). The Rust modules are declared from `src/lib.rs` (two `mod` lines at its end): a `mod.rs` no crate root reaches is treated as its own crate root, and `super::` paths from its children then resolve to `Unresolved` or to the root file — a resolver limitation, not fixed here. Rust's port has Ca = 1 (its `mod` declaration), so it imports three modules to reach D = 0.75.
- **P2.5 — changes outside the check.** `cupel-cli/tests/check.rs` pins its event-stream and dry-run tests to `--only static.complexity` (they assumed the only quick check); `describe.rs` and two text snapshots list `static.deps`; `cupel-lang/tests/{rust,go}_modules.rs` filter `deps_` edges out of the exact-graph tests and pin them in a test each (inline rows would trip `too_many_lines`), and `{python,ecma}_modules.rs` gained the rows inline; `golden/facts/*` and `golden/describe/*` regenerated and reviewed. The README has no check table yet; it is left for the P2 close-out so the parallel check steps do not collide on it.
- **P2.5 — open items.** Rust `mod` declarations are edges, so a child module that imports an item its parent defines or re-exports (`use crate::Error` with `Error` in `lib.rs`) forms a cycle with the parent; on the Cupel repository `cupel check --only static.deps --paths crates` reports 11 cycles (6 in Cupel's own crates, among them one component of 25 files in `cupel-core` and one of 22 in `cupel-lang`) and 4 `dep.unstable-abstract`. Telling declaration edges from use edges needs an `ImportEdge` field (a fact change in all five languages); until then P2.10 should expect `dep.cycle` noise on Rust crates. Abstract classes count as concrete types. On a narrowed scope Ca and cycles cover only edges from files in scope.
- **P2.5 — tests.** 820 in all (+24): `cupel-checks` 31 (`static_deps` 16 integration over synthetic graphs + 6 unit: Tarjan on a DAG, one cycle, nested cycles sharing a node, a self-loop and a 100 000-node chain; Martin arithmetic; rounding), `cupel-lang` +2 (`deps_` edges for Rust and Go), plus the five `golden/static.deps/<lang>.json` checked by the matrix test.
- **P2.6 — `tests.quality`** ([ADR-054](adr/CUPEL_DECISION_054.md), [ADR-055](adr/CUPEL_DECISION_055.md)). `crates/cupel-checks/src/tests_quality.rs`: tier 2, `quick`, `needs = [TestFrames, Symbols, NormTree]` (`NormTree` for the finding ID, as in ADR-042, and for duplicate setup), `wants = [Decisions]` (without it every kind but `test.logic` still reports). A test case joins its symbol by span start (`TestCase.span` start = `SymbolKey`, true for every fixture case in all five languages). Kinds and actions: `test.weak-assertion` (every assertion weak; `strengthen_assertion`), `test.no-assertion` (no assertion of any kind, so `#[should_panic]`, `pytest.raises`, `toThrow`, `assert.ErrorIs` — all `throws` — count; `write_test`), `test.mock-heavy` (`review`), `test.logic` (`refactor_extract`), `test.duplicate-setup` (`refactor_extract`). Every evidence object carries `remediation_mode` (`strengthen`, `add-assertion`, `decouple`, `simplify`, `extract-fixture`) and `actionability` (`mechanical | guided | judgment`), inlined string enums (`#[schemars(inline)]`, so no `$ref` reaches the evidence validator). Score `{value: assertions per test, unit: "assertion_density", n: tests}`, combined by the default `n`-weighted mean; metrics `assertion_density`, `weak_ratio`, `no_assertion`. All findings `warn`.
- **P2.6 — thresholds** under `[checks."tests.quality"]` (non-negative integers via `settings::Limits`): `mock_density_max = 50` (percent of doubles among doubles plus assertions), `mocks_min = 2`, `logic_max = 0`, `setup_statements_min = 3`. Logic = loops + cases/ternaries/catches + branches beyond one guard per assertion and per skip marker, because Go's standard-library assertion is a guarded failure call (`if got != want { t.Errorf }`) and `Decision` has no span to tell guards apart; table-driven Go tests (a loop) are reported (`logic_max = 1` per language relaxes it).
- **P2.6 — duplicate setup** (ADR-055) compares the leading statements of test bodies in `NormTree` (identifiers and literals erased, call names kept): within one file, tests whose first `setup_statements_min` statements are equal and which each have more statements than that form one finding on the first test, with every member's display name and line. Statements are the children of the root's last child (or of its only child, for Go's `statement_list`); a list holding a leaf is a single statement's parts and yields none. Declared setup blocks (`TestFile.setup`) are not compared: JavaScript/TypeScript hooks are not symbols and so have no tree.
- **P2.6 — fixtures and their ripple.** One file per language — `tests/quality_signals.rs`, `ledger_quality_test.go`, `tests/test_quality_signals.py`, `test/math.quality.test.js`, `src/math.quality.test.ts` — with exactly one test per kind plus clean tests (a strong equality and an expected panic/raise/throw). `golden/tests.quality/<lang>.json` also report the pre-existing fixture tests (weak and assertion-free tests, `math.test.*`'s mock-heavy spy test, Go's table-driven `TestClassify`). The new files changed every `golden/facts/<lang>/` golden that lists symbols, trees, decisions, sites, roots, imports, or frames (and Rust's `test-conventions`), `static.complexity`'s `n` and `scope.files`/`scope.symbols` (no complexity finding changed), and the `cupel-lang` tests that pin fixture contents: test-root counts in `{ecma,go,python,rust}_api.rs`, frame file lists in `{go,python}_tests.rs`, `rust_tests.rs`'s `test_files_for("src/lib.rs")`, and the exact import graphs in `{ecma,go,python,rust}_modules.rs` (the Rust one moved the new file's edges to a test of their own to stay under clippy's line limit). `cupel-cli/tests/check.rs`'s event-stream and dry-run tests now pass `--only static.complexity`, so they no longer depend on how many checks are registered; `describe.rs` and its two text snapshots list `tests.quality`.
- **P2.6 — not done.** The README has no check table yet (P2.1 did not add one); the recipe's step 8 is left for the parallel P2 merge, where one table can list every check. `kind`, `metric`, and `evidence` helpers are duplicated from `static_complexity.rs`; with several checks landing in parallel, moving them into a shared private module (as `finding` and `settings` are) is a merge-time refactor.
- **P2.6 — tests.** 814 in all (+18): `cupel-checks/tests/tests_quality.rs` 17 (per kind on hand-built `TestFrames`, `Symbols`, `NormTree`, and `Decisions`; the `throws` rule; guarded branches; wrapped statement lists and leaf lists; thresholds and their validation; scope; identity; score) and `rust_modules` +1.
- **P2.4 — algorithm** ([ADR-050](adr/CUPEL_DECISION_050.md)), in `crates/cupel-checks/src/static_clones.rs` and `static_clones/similarity.rs`. A symbol is compared when it spans at least `min_lines` lines and its `NormTree` has at least `min_nodes` nodes (leaves included). Its fingerprint set holds, for every non-leaf node, a hash of the node with its children and grandchildren (depth 2; labels FNV-1a, folded with the splitmix64 finalizer), repeated shapes numbered by occurrence so set Jaccard is multiset Jaccard; one edit changes only the fingerprints within two levels above it. MinHash: 128 functions `x ↦ mix(x ⊕ seedᵢ)`, fixed seeds. Banding is derived from the threshold: the most rows `r` with `b = ⌊128/r⌋` bands such that a pair at `threshold + 0.05` is missed with probability ≤ 10⁻⁶, which is `r = 5`, `b = 25` at 0.82. Candidates whose sizes cannot reach the threshold are dropped before the exact Jaccard (a sorted merge). A symbol never pairs with one whose span contains it. Connected components of the similarity graph become one finding each: two members `clone.pair` (evidence `jaccard`, `clone_type`, `threshold`, `left`/`right` with symbol, hash, lang, file, full span, lines, nodes), three or more `clone.cluster` (`members`, `pairs` among them, `min_jaccard`, `max_jaccard`, `threshold`); action `refactor_extract`, severity `warn`. ID: first member's hash plus a site hash folding the other members' hashes; a group is reported when any member is in scope, so the repro (`--symbols <first hash>`) reproduces it. Score `{value: similar pairs, unit: pairs, n: symbols compared}`, summed across families. Settings `threshold` (0 < t ≤ 1), `min_lines`, `min_nodes` are read by the module itself (`settings::Limits` handles integers only).
- **P2.4 — family comparison** ([ADR-051](adr/CUPEL_DECISION_051.md), commit `66d894b`, additive, own commit). The scheduler made one task per language with that language's facts, so JS↔TS pairs were impossible. `Check::spans_family()` (provided, default `false`) now makes the scheduler group the supported languages of a family into one task (first language in ID order; scope the union; config that language's slice), serving the other members' facts through `Facts::family::<F>()` (`insert_sibling`, `insert_shared_sibling`). `static.clones` returns `true`. Tests: `cupel-core/tests/scheduler.rs` +4, `cupel-fixtures/tests/clones_family.rs` (JS and TS fixture repos side by side produce a cluster spanning both languages; Go beside Python never mixes). A per-language override under `[checks."static.clones".lang.typescript]` is ignored when JavaScript is also in scope.
- **P2.4 — clone types.** `clone_type` is 2 when the two normalized trees are equal and 3 otherwise. Type 1 cannot be told from type 2 with the facts a check may read: `NormTree` keeps no identifier or literal text. Follow-up: a raw-token digest on `NormalizedSymbol` (NormTree version 2, every language producer) would let `clone_type` read 1.
- **P2.4 — fixtures and goldens.** `repos/<lang>/{src/clones_settlement.rs, clones_settlement.go, src/ledger/clones_settlement.py, src/clones_settlement.js, src/clones_settlement.ts}` hold a type-2 pair (`settle_invoices`/`reconcile_payments`, renamed with a changed literal) and a type-3 pair (`order_lines`/`refund_lines`, one extra statement, Jaccard 0.84–0.89); the P1.4 `twins` files already form a type-2 cluster in every language (Go's operator and call twins join it at 0.83). Every other fixture function produces nothing. The Rust file is not declared in `lib.rs` (the walk is file-based); the Go file imports nothing, but `go_modules` lists it as a package file. Each `golden/static.clones/<lang>.json` has two pairs and one cluster. New fixture functions stay within every `static.complexity` limit; its goldens change only in `score.n`, `scope.files`, and `scope.symbols`. `golden/facts/*`, `golden/describe/*`, and the two `describe` text snapshots were regenerated. `cupel-cli/tests/check.rs`'s event-order and dry-run tests now pass `--only static.complexity` (they assumed one registered check); `describe.rs` expects both checks.
- **P2.4 — performance.** 10,000 synthetic trees (about 120 nodes each, 100 planted near-copies): fingerprints plus search take about 0.2 s in a release build on an Apple M2 (8 cores), about ten times that in a debug build. `similarity::tests::ten_thousand_symbols_are_searched_in_linear_work_and_under_a_second` asserts every planted pair is found, exact comparisons ≤ 2n (deterministic proof the candidate stage stays linear), and a wall-clock bound of 1 s in release (`cargo test --release -p cupel-checks`) or 20 s in debug (`make test`, which shares the machine with other builds).
- **P2.4 — recall.** `search_never_misses_a_pair_above_the_recall_margin` (proptest, 256 cases): random sets whose Jaccard is at least 0.87, among up to eight unrelated sets, are always found, and every reported similarity is exact and ≥ the threshold. A miss has probability ≤ 10⁻⁶ per pair by construction, so the test is not deterministic, only overwhelmingly likely to pass.
- **P2.4 — `explain` and text.** `explain <finding-id>` prints the excerpt around the first member and the full evidence (both spans); there is no side-by-side rendering of the second member's source yet, which needs `explain` to read `evidence.right` (a CLI change). The text renderer's one line is the title, which names both symbols and the second's location.
- **P2.4 — tests.** 826 in all: `cupel-checks` +24 (`static_clones` 12 against synthetic facts, `similarity` 12 unit including the property and performance tests), `cupel-core` +4 (`scheduler`), `cupel-fixtures` +2 (`clones_family`). Not done: README check table (the README has none yet and still says no checks exist; left for the phase's README pass).
- **P2.7 — base-side facts** ([ADR-056](adr/CUPEL_DECISION_056.md), commit 55d96e0, additive). A check opts in with the provided `Check::base_needs(&self) -> &[FactKind]` (default empty; each kind must also be in `needs`) and reads `facts.base::<F>()` (`UndeclaredFact`/`AbsentFact` like `get`) and `facts.range() -> Option<&Range>` (`Range { commit, changes: Vec<git::Change { path, kind: ChangeKind::{Added, Modified, Deleted} }> }`, every changed path after `--paths`, including deleted and non-source files; set for every check under a range). `Facts::insert_base_shared`/`set_range` are the scheduler's side. `git` gained `changes_since`, `changes_between` (`diff --name-status --no-renames`; untracked files are `Added`) and `show(rev, path)`; `changed_*` are now their path lists. `ResolvedFiles` gained `changes`. `BaseTree::checkout(range, &git, &scratch, owned)` writes the base versions of the changed, language-owned, not-added files to `.cupel/work/<run>/base/` (`BaseTree::planned` for dry runs and for runs no selected check reads the base of); `Job` gained `base: Option<BaseTree>`; the scheduler keeps a second fact store rooted there and drops its producer findings. A check with `base_needs` on a scope without a range is skipped per language with the new `SkipReason::NeedsRange` (`needs-range`, `see: cupel check --changed --only <id>`). The CLI (`check.rs`: `base_files`, `base_tree`, `languages_of` over head and base files) counts a language as in scope when it owns a changed file on either side. The head side is still the working tree (ADR-040), so `--base A --head B` compares `A` with the working tree unless it is at `B`.
- **P2.7 — `tests.weakening`** ([ADR-057](adr/CUPEL_DECISION_057.md)). Tier 2, quick, `needs = base_needs = [TestFrames]`, no settings, all five kinds `fail` with action `restore_test` and no `symbol` (IDs from `FindingId::for_subject` on `<file>::<test>` or the snapshot path), metric `weakened`, score `{findings, "weakened", tests compared}` summed across languages. Matching is file + name (+ repeat index), then name across files; a snapshot path (`__snapshots__`/`snapshots` segment, `.snap`/`.ambr`) the range modified is attributed to changed tests by directory and file stem or test name, located at the snapshot when no changed test claims it, and not reported when a claiming test is new (the file may only have grown). A range that changes only non-source files has no language in scope, so a snapshot-only commit is not seen unless the code it records changed too.
- **P2.7 — matrix and goldens.** `golden/tests.weakening/<lang>.json` are the tree-scope `needs-range` skip; `matrix::verify_supported` accepts exactly that skip for a check with `base_needs` (and requires it), and `run_with_declared_facts` serves such a check the fixture as both sides of an empty range. Real behavior: `cupel-fixtures/tests/weakening.rs` builds a git history per language (base suite + snapshot + source file on `main`; branch `work` with one commit per kind, then a strong test added) and runs the binary per commit with `--base/--head` (checked out at head), with `--changed` (all five, snapshot tied to its test), and on a tree (skip, text rendering); plus a deleted-file/moved-test history and a grown jest snapshot. `cupel-checks/tests/tests_weakening.rs` covers matching, combinations, attribution, settings, and `combine` over synthetic frames.
- **P2.7 — files outside the check.** `cupel-core`: new `base.rs`; `check.rs` (`base_needs`), `fact.rs`, `git.rs`, `scope.rs`, `schedule.rs`, `envelope.rs` (`NeedsRange`), `lib.rs`; tests `git.rs` (+3), `scheduler.rs` (+2, `Job { base: None }`), `contract.rs`, two schema snapshots. `cupel-cli/src/check.rs`. `cupel-fixtures/src/matrix.rs`. Because a second quick check now exists: `cupel-cli/tests/check.rs` pins two tests to `--only static.complexity`, `cupel-cli/tests/describe.rs` expects the new matrix row, and the `describe` matrix/language goldens and text snapshots gained it. Tests: +25 (`cupel-core` 5, `cupel-checks` 13, `cupel-fixtures` 7). The README still has no check table (none existed after P2.1); the phase-end README pass adds it.
- **P2.9 — metrics** ([ADR-060](adr/CUPEL_DECISION_060.md), [ADR-062](adr/CUPEL_DECISION_062.md)). `MetricSpec` gained `per: scope | symbol | module | group`; `scope` metrics are gate keys (`Config::validate_registered` now rejects a per-target metric as one) and are reported in the new `CheckResult.metrics` map (`#[serde(default)]`, so stored envelopes still read), which the scheduler combines across languages with the new provided `Check::combine_metric` (default: largest). Per-target values go through the new provided `EventSink::measure(Measurement { metric, value, target: Target { display, symbol, location } })`, a no-op for every sink but the scheduler's, which keeps them in `Outcome.measurements` (`Measured { check, lang, measurement }`); they never reach `--events`. `static.complexity` fills its five `_max` metrics and measures `cyclomatic` and `cognitive` per symbol (both `per: symbol`).
- **P2.9 — what a check must emit.** For the gate: declare each gate key as a `MetricSpec { per: MetricPer::Scope, .. }` and put its value in the `CheckResult.metrics` the check returns (override `combine_metric` when the largest per-language value is wrong, e.g. a ratio or a count); otherwise every rule on it is `skipped` and fails the gate. For `survey`: declare a `MetricSpec` with `per` `symbol`, `module`, or `group` and call `sink.measure(...)` once per target in scope. Every `CheckResult { … }` literal needs `metrics` (an empty `BTreeMap` when there is nothing to report) and every `MetricSpec { … }` literal needs `per`.
- **P2.9 — gates** ([ADR-060](adr/CUPEL_DECISION_060.md)). `cupel_core::{gate_for(&GateConfig, ScopeMode), evaluate_gate(name, rules, &Envelope), apply_gate(&mut Envelope, GateResult)}` in `cupel-core/src/gate.rs`. `[gate.changed]` for `changed`/`range`/`since-baseline`, `[gate.tree]` otherwise; evaluated by default when that table exists, never with `--no-gate`; `check --gate` without the table is exit 2 on `--gate` before any check runs. `GateRule.result` is the new `RuleOutcome` (`pass | fail | skipped | unsupported`): a rule whose check did not complete (not selected, skipped everywhere, `error`, `partial`) or did not report the metric is `skipped`; `none` counts unsuppressed `warn`/`fail` findings (`actual` is the count); `no-breaking` is `unsupported`; `%` thresholds divide by 100; an empty scope passes every rule vacuously. Any rule not `pass` fails the gate, which turns `pass` into `fail`/exit 1 and leaves `partial`/`error` alone. In the CLI, `app::graded` calls `check::grade` after suppressions and deltas for `check` and `survey`; `survey` never gates (`survey --gate` is exit 2).
- **P2.9 — `next_actions`** ([ADR-061](adr/CUPEL_DECISION_061.md)) moved from `schedule.rs` to `cupel-core/src/actions.rs`: `next_actions(&Envelope, rerun: &str)`, computed from the graded envelope. Bands: `fail` findings, gate rules that could not pass (`rerun` with `<verb> <scope flags> --only <check>`, or `configure` for `unsupported`), `warn` findings, other skipped checks (`timeout`/`budget` → `rerun`, `unavailable` → `install_tool` with `cupel doctor`, `unconfigured` → `configure`; `unsupported` → none), `info` findings; within a band new-since-baseline first, then tier, then envelope order. Suppressed findings get none. De-duplicated by action kind plus location (else symbol hash, else ID) or check; at most `NEXT_ACTIONS = 10`.
- **P2.9 — `survey`** ([ADR-062](adr/CUPEL_DECISION_062.md)). `cupel-cli/src/survey.rs` calls `check::run` (the shared pipeline) and `CheckRun::into_envelope`, then sets the new nullable `Envelope.rankings` (`{top, dimensions[{check, metric, per, summary, measured, entries[{rank, value, lang, target}]}]}`) from `cupel_core::rankings(manifests, &measurements, RANKED)`: one dimension per non-`scope` metric of every check with an entry in `checks[]`, highest values first (ties by display name, file, line), `RANKED = 10`. Null for dry runs and every other verb. The text renderer prints a `rankings` block after `checks`, rule outcomes in the gate block, and each check's metrics.
- **P2.9 — shared files.** `app.rs` (the `survey` arm; `graded` takes the configuration and calls `check::grade`; the slow-run test uses `tests`), `check.rs` (`grade`, `rerun`, `scope_request`, the `--gate` precondition, `CheckRun::{into_envelope, rankings}`), `registry.rs` (`survey`, `--gate`, `--no-gate` un-pended; `survey`'s example exits 0), `run.rs` (`shell_quote` is `pub(crate)`; `rankings: None`), `main.rs`, `render/{text.rs, text/description.rs, fixture.rs}`, `cupel-checks/src/static_complexity.rs` (metrics and measurements only). The pending-verb stand-in in `tests/{format,render,run_infrastructure,describe}.rs` and `app.rs` moved from `survey` to `tests` (P3.3); `exit_codes` expects 3 pending verbs and gained the exit-1 trigger (a `[gate.tree]` rule on `static.complexity.cyclomatic_max`). Every golden gained `"rankings": null`; `static.complexity` goldens gained `checks[].metrics`; the `run` and `plugin` schema snapshots changed.
- **P2.9 — goldens.** `golden/survey/{go,javascript,python,rust,typescript}.json` (`tests/survey_goldens.rs`, languages from the registry) hold every registered check's results, findings, and rankings over the fixture repositories, so they change whenever a check lands or a fixture grows; regenerate with `UPDATE_GOLDENS=1 make test` and review.
- **P2.9 — tests.** 831 in all (+35): `cupel-core` `gate` 10, `next_actions` 7, `rankings` 2, `scheduler` +1 (two new; the old `next_actions` test moved into `next_actions`), `config_registry` +1 case; `cupel-checks` +2; `cupel-cli` `gate` 6, `survey` 5, `exit_codes` +1; `cupel-fixtures` `survey_goldens` 1.
- **P2.9 — open items.** Per-target measurements are not stored; `explain <check-id>` still shows only the score (ADR-062 Consequences). A ranking dimension where lower is worse (P3 mutation score) needs a direction on `MetricSpec`. `no-breaking` waits for P6.2 to define a breaking finding. The P2.3–P2.8 checks must fill `metrics` and call `measure` as above to take part in gates and rankings.
- **P2.9 — integration — core** ([ADR-063](adr/CUPEL_DECISION_063.md)). Reconciling P2.9 with P2.4/P2.7: `Scheduler::run` calls `run_task(job, store, base, task, run.sink(&task.lang, sink))` (P2.7's base fact store plus P2.9's per-language measuring sink); a family-wide task's measurements carry the task's language. `next_actions` gives a `needs-range` skip a `rerun` whose command is the skip's `see` (`cupel check --changed --only <id>`), and a skipped gate rule on such a check reuses that command, de-duplicated with the skip. `Check::combine_metric(name, results: &[&CheckResult])` now receives the per-language results that reported the metric (score and whole metric map), so ratios can be weighted; the default is still the largest.
- **P2.9 — integration — what each check emits.** `static.crap`: `max` (largest), `mean` (weighted by `score.n`), per-symbol `crap`. `static.clones`: `pairs`, `clusters` (summed), `max_jaccard` (largest) over the reported groups, per-group `cluster_size` (display `<first> and N others`, no symbol, first member's location). `static.deps`: `cycles`, `violations` (summed), `max_distance` (largest), per-module `distance` and `coupling` (Ca + Ce), per-group `cycle_size` (display the cycle path). `tests.quality`: `assertion_density` (weighted by tests), `weak_ratio` (weighted by assertions = density × tests), `no_assertion` (summed), per-symbol `mock_density` (higher is worse; assertion density waits for a ranking direction). `tests.weakening`: `weakened` (summed). `static.dead-code`: `dead_symbols`, `dead_exports` (summed); no per-target metric for either.
- **P2.9 — integration — shared helpers.** `cupel-checks/src/manifest.rs` (`kind::<E>(name, action, summary)`, `metric`, `ranked(name, per, summary)`, `evidence`) replaces the copies in all seven modules; `metrics.rs` gained `symbol_target`, `largest`, `sum`, `weighted_mean`, `scored`. `cupel-checks/tests/support/mod.rs` holds `Recorded` (a sink keeping findings and measurements), `language`, and `combined` for `combine_metric` tests.
- **P2.9 — integration — gates and goldens.** `cupel-cli/tests/gate.rs` runs the binary on Python projects: `[gate.tree]` `static.crap.max < 30` and `static.deps.cycles == 0` fail (42, 1) then pass (6, 0); `[gate.changed]` `tests.weakening = "none"` fails on a removed assertion (actual 1) and passes once a test is added (0); in `[gate.tree]` that rule is `skipped` with one rerun action over `--changed`. `survey_goldens` now also asserts that every check declaring a per-target metric ranks something and every check that ran reports exactly its declared gate keys. Every check golden gained `metrics`; the `tests.weakening` goldens gained the rerun action; `golden/survey/*` list every check's results and nine dimensions. The P2.9 tests that read `checks[0]`/`would_run[0]` as `static.complexity` now find it by ID or pass `--only`. 979 tests in all; the integration added 18 (`cupel-checks` 12 metric, measurement, and `combine_metric` tests; `cupel-core` `scheduler` +1, `next_actions` +2; `cupel-cli` `gate` +3), beside the changed assertions above.
- **P2.9 — integration — open items.** A `[gate.tree]` rule on `tests.weakening` can never pass (rejecting it at load is a follow-up). The README still has no check table.

---
- **P2.10 — scoping.** Cupel's `cupel.toml` (from `cupel init --write`, then edited) excludes `crates/cupel-fixtures/**` through the new `[scope] exclude` table ([ADR-065](adr/CUPEL_DECISION_065.md)): gitignore-style globs (a pattern without `/` matches a name at any depth), applied to root detection (which never descends into an excluded directory), the file walk, and `--changed` lists. `vendor`, `node_modules`, `target`, and `.cupel` are always excluded. So the dogfood grades the six product crates.
- **P2.10 — initial findings** (`cupel check --depth quick`, binary at the start of P2.10, product crates): 610 — `test.logic` 162, `clone.pair` 74 and `clone.cluster` 44, `test.weak-assertion` 115, `test.no-assertion` 66, `function.long` 55, `test.duplicate-setup` 35, `complexity.high` 34, `params.many` 17, `dep.cycle` 6, `nesting.deep` 2.
- **P2.10 — Cupel bugs fixed, each with a fixture or regression case.** One `checks[]` entry per check when every language skips ([ADR-064](adr/CUPEL_DECISION_064.md); scheduler regression test). `ModuleGraph` v2 tags each edge `import | declaration | re-export` in all five languages and `static.deps` cycles ignore declarations ([ADR-068](adr/CUPEL_DECISION_068.md); `deps_tree` fixtures, conformance inventories, goldens) — this removed the Rust parent/child `mod` cycles. A loop over a table written in place is parametrization, not test logic ([ADR-069](adr/CUPEL_DECISION_069.md); a table test and a computed-loop test in every language's quality fixture), and in Rust a loop over a SCREAMING_CASE constant too ([ADR-071](adr/CUPEL_DECISION_071.md); `LISTED`). Rust `assert!(a == b)`, `assert!(x.is_none())`/`is_empty()`, `assert_…` helpers, and `assert_cmd`'s `.assert()` are assertions of the right strength ([ADR-070](adr/CUPEL_DECISION_070.md)), and an `assert_…` helper called with a turbofish counts too (`an_assertion_helper_called_with_a_turbofish_is_an_assertion`). `static.complexity` counts a `match`/`switch` once in cyclomatic complexity ([ADR-072](adr/CUPEL_DECISION_072.md); a 12-arm `month` dispatch in every fixture). `static.crap` without a `CoverageMap` passes with a null score and no findings instead of guessing coverage ([ADR-067](adr/CUPEL_DECISION_067.md), superseding ADR-048's no-coverage mode; `no_coverage` setting removed; unit test on a hand-built `CoverageMap`). A `[gate.tree]` rule on a check that needs a range (`tests.weakening`) is a configuration error at load, exit 2, pointing to `[gate.changed]` ([ADR-066](adr/CUPEL_DECISION_066.md); binary test).
- **P2.10 — code fixed.** Every `dep.cycle` (module layering in `cupel-core`: leaf `fact.rs`, `result.rs`, `status.rs`, `config/error.rs`; cycles in `cupel-store`, `cupel-cli`, `cupel-lang`), every `complexity.high`, `function.long`, `params.many`, and `nesting.deep` (long verbs run through `Invocation`; scheduler, scope, schema, store, resolver, and extractor functions split), and every test-quality finding (weak assertions became exact comparisons of whole values; loops and branches moved into named helpers; repeated arrange steps became shared helpers in each crate's `tests/support`). `NormNode::new`/`leaf` replace hand-built trees.
- **P2.10 — final findings** on the same command: 99, all `static.clones` (69 `clone.pair`, 30 `clone.cluster`; 213 similar pairs). `static.complexity`, `static.deps`, `static.dead-code`, and `tests.quality` report nothing; `static.crap` passes with a null score; `tests.weakening` skips on a tree.
- **P2.10 — suppressions.** None: `.cupel/suppressions.toml` does not exist. The only two deferred limitations it may hold (below) produce no warn finding on the tree today (`dead.export` is info and there are none), so nothing needed suppressing.
- **P2.10 — gates.** `[gate.changed]`: `tests.weakening`, `static.complexity`, `static.clones`, `static.deps`, `static.dead-code`, `tests.quality` all `none`, plus `static.complexity.cyclomatic_max <= 10` and `static.deps.cycles == 0`. `[gate.tree]`: the same without `tests.weakening` (ADR-066), but `static.clones` as a ratchet, `static.clones.pairs <= 213` and `static.clones.clusters <= 30`, because the 99 remaining groups (cross-language test scaffolding and same-shape data variants, some across crate boundaries that share no test-support crate) were not all removed in this step. `static.crap.max` joins both gates with coverage (P3.2). `cupel check --gate` exits 0.
- **P2.10 — changed gate open.** `cupel check --changed --gate` against `main` exits 1: `main` is the end of P1, so the range is all of P2. Beyond the clone groups it reports `tests.weakening` findings that are P2's deliberate test changes: 19 `snapshot.rewritten`, 7 `test.deleted`, and 54 `assertion.removed`, almost all where several weak assertions were folded into one exact comparison of a whole list (the weakening check counts assertions and cannot see that one strong assertion replaced several weak ones). The gate is expected to pass once P2 merges to `main`; the count-based `assertion.removed` is a follow-up below.
- **P2.10 — make dogfood and CI.** `make dogfood` runs `cupel check --changed --gate --format text` (`DOGFOOD_SCOPE=tree` drops `--changed`); the CI `dogfood` job checks out with `fetch-depth: 0`, runs the tree gate on `main` and the changed gate on branches (documented in the Makefile comment). Until P2 is on `main`, `make dogfood` on this branch fails for the reason above; `make dogfood DOGFOOD_SCOPE=tree` passes.
- **P2.10 — tests.** 1013 in all; `make check` green.
- **P3 follow-up list.** (1) A `References` fact so `static.dead-code` decides reachability per call, not per module ([ADR-058](adr/CUPEL_DECISION_058.md)). (2) A raw-token digest on `NormTree` so `static.clones` can tell type 1 from type 2 ([ADR-050](adr/CUPEL_DECISION_050.md)). (3) Bring `static.clones` on the Cupel tree to zero and replace the ratchet with `none`. (4) `tests.weakening`'s `assertion.removed` should weigh strength, not count, so folding weak assertions into one exact one is not a weakening. (5) `static.crap.max` in both gates once coverage exists (P3.2).
- **P2 — close-out** (2026-10-14; each DoD line checked against the release binary, `cargo build --release`, and `make check`). (1) *Goldens in all five languages:* met. `crates/cupel-fixtures/golden/` holds one envelope per (check, language), 35 in all; `describe matrix` marks all 35 cells `supported`, so no skip assertion is needed; the matrix test (`every_pair_has_its_golden_or_its_skip_and_no_golden_is_stale`, `every_check_runs_given_only_its_declared_facts`) passes. (2) *`cupel survey --depth quick` on the Cupel repo in < 5 s:* met. 0.64 s wall (five warm runs, 0.64–0.66 s) and 0.69 s on a fresh clone with no `.cupel/`; 216 files, 2816 symbols, exit 0. (3) *`cupel check --changed --gate` passes in CI on the Cupel repo:* not met on this branch, by construction, and met by the mechanism that replaces it. Against `main`, which is still the end of P1, the range is all of P2 and the changed gate exits 1: `tests.weakening` 80 (54 `assertion.removed`, 19 `snapshot.rewritten`, 7 `test.deleted`; P2's deliberate test rewrites, see "P2.10 — changed gate open") and `static.clones` 96 (67 pairs, 29 clusters, the groups the tree ratchet holds), plus 2 `dead.export` at info. The tree gate passes (`make dogfood DOGFOOD_SCOPE=tree` exit 0; clones at the ratchet, 213 pairs and 30 clusters). After the merge, CI runs the tree gate on `main` and the changed gate on every branch against the new `main`, which is where this line is satisfied. (4) *`describe` covers every shipped check and finding kind:* met. `describe matrix` lists the seven checks and `describe <id>` exits 0 for each; the 23 finding kinds in the manifests are exactly the `valid` list of `describe finding <unknown>`, and `describe finding <kind>` exits 0 for each and names its check. The new `every_finding_kind_in_the_check_table_is_described` test in `crates/cupel-cli/tests/readme.rs` makes the second half mechanical. `describe --all` carries the CLI, verbs, and schemas but not checks; the check registry is reached through `describe matrix` and `describe <id>`. (5) *README:* met. Rewritten from the real binary: usage for every built verb, captured output (a `check --changed --gate` envelope as JSON and text, `survey`, `explain`, `baseline diff`, `suppress`) from a small Python demo repository with a branch that adds a complex function, a copied function, a loosened assertion, and a skipped test; gate configuration and rule grammar; the agent loop and exit codes. The exit-code table, check table, and support matrix are generated by `crates/cupel-cli/tests/readme.rs` from `cupel describe` and `make readme` rewrites them; `make test` fails while they are stale ([ADR-073](adr/CUPEL_DECISION_073.md)). This closes the "no README check table" items in the P2.3, P2.6, and P2.9 notes.
- **P2 — close-out: found while writing the README, not fixed.** (1) `cupel describe --agent` is P8.3 and exits 2 with `cli.not-implemented`, so the README's Agents section is built on `describe --all`, `describe <check>`, `--describe`, and `--dry-run`. (2) `--since-baseline` is described as `pending P2.1`, but P2.1 is complete and did not build it; the flag is accepted and ignored (a tree scope runs, exit 0), as are the other pending scope flags (`--budget`, `--oracle`, `--cases`). Its owning step needs choosing, and a pending flag should arguably be a `cli.not-implemented` usage error as `--format sarif` is. (3) Under `--changed` or `--paths`, `static.clones` sees only the symbols in scope (facts are produced for the scoped files), so a new function copied from an unchanged one is not reported by `check --changed`; `survey` and tree-scope `check` find it. This is the clones form of the P2.8 narrowed-scope item, and ADR-050's "a group is reported when any member is in scope" holds only when every member's file is in scope. (4) `explain <check-id>` still returns only the stored score (P2.9 open items), and `explain` of a `clone.pair` shows only the first member's excerpt (P2.4 notes).
- **P2 — close-out: tests.** 1015 in all (two new in `readme.rs`); `make check` green; `make dogfood DOGFOOD_SCOPE=tree` exit 0.

### P3 — Coverage and mutation

**Goal:** Tier 2–3 evidence: real test execution, coverage ingestion, per-test maps, impacted-test selection, coverage-gated differential mutation testing, flakiness detection, and the mandatory exit-4 refusal when the baseline suite is red.

**Status:** in progress (2026-10-14)

External tools used here (`cargo-llvm-cov`, `cargo-nextest`, `go`, `pytest`, `pytest-cov`, `vitest`, `jest`) are exercised for real in integration tests when present and reported as skipped when absent. Parsers are always tested against captured real output committed as fixtures.

#### P3.0 Decentralized registration and registry-derived tests

**Status:** in progress (2026-10-14) · [ADR-074](adr/CUPEL_DECISION_074.md)

P3 fans out to parallel worktrees, each adding a check or a producer. Before it does, remove every line that two such additions would both edit: the `register()` list in `cupel-checks`, the generic-producer table in `cupel-lang`, hand-maintained lists of check IDs, finding kinds, fact kinds, and producers in tests, goldens that render every check at once, and duplicated test helpers. Afterwards, adding a check is literally one module plus goldens (§2.8), and adding a tool producer is one module in `cupel-adapt`.

- Checks register themselves from their own module (`inventory::submit!`); `cupel_checks::register` is derived from the inventory, sorted by check ID, and still panics on a duplicate ID. Write **ADR-074** (supersedes ADR-001's registration consequence): the mechanism, the trade-offs, and what a new check may touch.
- The same mechanism for producers: `cupel-lang`'s generic producers submit themselves from their fact modules; `cupel-adapt` gains `register(&mut ProducerRegistry)` over its own inventory, which every registry builder (CLI catalog, matrix harness) calls now, so a P3 tool producer is one file.
- Tests derive their expectations from the registries: every registered check and finding kind describes; every `supported` cell has a golden; `cells.len() == checks × languages`. No test lists check IDs, finding kinds, fact kinds, or producers.
- One golden directory per check: everything a golden pins about one check lives under `golden/<check-id>/`; goldens that render the whole registry (survey, describe matrix and language) keep only what no single check owns.
- Test helpers duplicated across test files (facts, contexts, fixture paths, temporary git repositories, running the binary) move into one shared module per crate, or `cupel-fixtures` where several crates use them. Clone counts before and after go in Notes; the `static.clones` tree ratchets may only go down.

*Tests first:* a test that `lib.rs` names no check type (fails while `register()` lists them); a duplicate submission panics with "native check IDs are unique"; a producer submitted from a test binary reaches `cupel_adapt::register`; registry-derived describe coverage fails when a registered check is not described.

*Done when:* a new check touches only its module file, its `mod` line, its fixtures, and `golden/<check-id>/`; a new tool producer touches only its module file and its `mod` line; `make check` and `make dogfood DOGFOOD_SCOPE=tree` exit 0.

#### P3.1 Fact: `TestResults` — runner producers and baseline test run (exit 4)

**Status:** not started

- `Runner: Producer<TestResults>` trait (design §8.7); producers for `cargo test`, `cargo nextest`, `go test -json`, `pytest`, `vitest`, `jest`, registered through the P1.9 toolchain table; `list()` and `run(selection, env, timeout)` → `TestResults` (per-test outcome, duration). Each producer = a format parser (tested on committed real output) + an invocation.
- Baseline test run executed once in the real tree before any producer that needs it; red baseline → `status: error`, exit 4, no scores produced. `test_results` index table populated from the fact.
- `doctor` gains tool presence/version checks for every producer the configured checks' needs resolve to.

*Tests first:* parser fixtures for each runner's output format; a fixture with one failing test → `check --only tests.mutation` exits 4 and emits zero mutation findings; `[lang.python] runner = "unittest"` selects that producer.

#### P3.2 Fact: `CoverageMap` — coverage producers, `tests.coverage`, full `static.crap`

**Status:** not started

- `Coverage: Producer<CoverageMap>` trait; format parsers for LCOV, Cobertura XML, Istanbul JSON, go coverprofile, JaCoCo XML → `CoverageMap` (line + `branches: Option<_>`). Invocations for Rust (`cargo llvm-cov --lcov`), Go (`go test -coverprofile`, `branches: None`), Python (`pytest --cov --cov-report=lcov`), JS/TS (`vitest --coverage`, `jest --coverage`, `c8`). Ingest existing reports when fresher than sources (mtime + content hash).
- `tests.coverage` check: `needs = [Symbols, CoverageMap]`; `coverage.uncovered-symbol` always; `coverage.uncovered-branch` only when `branches` is present, else the check result notes `branches: unavailable` — it learns this from the fact, not from the language.
- `static.crap` gains `wants = [CoverageMap]`; `--no-coverage` retained.

*Tests first:* each parser against committed real reports; `CoverageMap` property (line hits are non-negative and symbol coverage ∈ [0,1]); CRAP golden with known coverage; Go golden shows no branch findings and the documented note.

#### P3.3 Fact: `PerTestMap` — per-test coverage and `tests --impacted`

**Status:** not started

- `PerTestMap` producers: nextest + llvm-cov per-test profiles (Rust), `go test -coverprofile -run '^TestX$'` per test (Go), pytest-cov `--cov-context=test` (Python). None registered for `ecma` — `describe matrix` shows `PerTestMap`-needing behavior as unsupported for JS/TS, and `tests --impacted` falls back to `ModuleGraph` impact with `per_test_map: false` echoed. Write **ADR-006**.
- Inverted index `line → tests`; `tests --impacted` returns `{id, file, reason}` plus a ready-to-run `command` per runner; says when it fell back.

*Tests first:* a fixture with two tests covering disjoint functions — changing one function yields exactly one impacted test; the JS fixture takes the fallback path and says so.

#### P3.4 Fact: `MutationOutcomes` — the mutation engine and `tests.mutation`

**Status:** not started

`cupel-mutate` is `Producer<MutationOutcomes>` with `needs = [MutationSites, CoverageMap, TestResults]`, `wants = [PerTestMap, ModuleGraph]`, obtaining a `Runner` from the registry. `tests.mutation` is a check with `needs = [Symbols, MutationOutcomes]`, `wants = [TestConventions]`, that only scores and writes findings. Built in sub-steps, each with its own failing test first:

1. **Sites → mutants**: from `MutationSites`, generate mutant source for each; `mutants` index table.
2. **File-copy overlay**: reflink (APFS/btrfs/XFS) else hardlink tree + private copy of mutated file plus every file `ModuleGraph` marks as a relative importer of it. Mutants of one file run concurrently, files sequentially.
3. **Coverage gating**: uncovered sites are recorded `uncovered` and never executed.
4. **Per-test selection**: only tests covering the mutated line run when `PerTestMap` is present; full suite otherwise.
5. **Timeouts**: `timeout_factor × baseline_duration(test)`, floor 2 s → `timeout` outcome.
6. **Store-based differential reruns**: outcome reused when `(symbol_hash, site_hash, suite_hash, toolchain_hash)` unchanged; scores recomputed from the store.
7. **Ordering**: last run's survivors → sites in changed symbols → rest.
8. **`tests.mutation` check**: `mutant.survived` (with `from`, `to`, `operator`, `covering_tests`, `nearest_test_file` from `TestConventions`), `mutant.timeout`, `mutant.uncovered`, `mutant.equivalent?` per **ADR-004**. Score = killed / (killed + survived).
9. Depth mapping: `standard` = changed symbols, `deep` = touched files, `exhaustive` = tree — applied by the engine's `plan()`.

*Tests first:* a fixture per language with a function and a deliberately weak test suite where the surviving set is known exactly; golden asserts the exact survivors; a second run with no changes executes zero mutants (differential); a change to one test re-executes only the sites it covers; `tests.mutation` run against a hand-built `MutationOutcomes` fact (no engine) produces the expected findings — proving the check is independent of how outcomes were produced.

#### P3.5 `tests.flaky`

**Status:** not started

- A flake engine (`Producer<TestResults>` variant producing repeated runs, registered as `engine.flake`) reruns impacted tests N times (default 3 at `standard`) with shuffled order, varied seeds, varied parallelism. `tests.flaky` check consumes the repeated results and `TestFrames`. Kinds `test.flaky`, `test.order-dependent`; action `investigate_flake`.

*Tests first:* a fixture test that depends on a global mutated by another test → `test.order-dependent`.

#### P3.6 Dogfood: mutation gate

**Status:** not started

- `[gate.changed] "tests.mutation.score" = ">= 0.80"` on the Cupel repo. Fix Cupel's own tests until it passes. Record the initial score and what was fixed in Notes.

#### P3 Definition of Done

- All goldens pass; real-tool integration tests pass on a machine with the tools installed (CI installs them).
- `cupel check --changed --depth standard` on a 10-symbol change in the Cupel repo completes in ≤ 3 min (record in Notes).
- Exit 4 is enforced for every check that depends on test execution.
- README: coverage/mutation usage, `tests --impacted`, exit 4, the mutation performance stack.

#### P3 Notes

- **P3.0 — mechanism** ([ADR-074](adr/CUPEL_DECISION_074.md)). `inventory` 0.3.25 (`inventory::submit!` in the module, `inventory::iter` in the entry point). Checked before choosing, on the pinned 1.92 toolchain: it builds under the workspace's `unsafe_code = "forbid"` and collects every submission across crates in debug and in a release build with `lto = true`. `linkme` was rejected because its `#[link_section]` statics expand at the use site, which `forbid(unsafe_code)` refuses ("declaration of a static with `link_section`"); a build script generating `mod` lines through `include!` was rejected because `rustfmt` never visits modules declared that way, so check files would leave `make lint`'s format gate. Three submission types, each with one entry point: `cupel_checks::NativeCheck` → `cupel_checks::register` (sorted by check ID; still panics "native check IDs are unique"); `cupel_lang`'s crate-private `generic::GenericProducer` → `generic::register{,_except}` (sorted by fact kind; the `GENERIC` table in `generic.rs` is gone, each fact module submits its own); `cupel_adapt::ToolProducer { id, register }` → `cupel_adapt::register` (sorted by ID; panics "tool producer IDs are unique"), which `Catalog::builtin` and the matrix harness's `Registries::builtin` call now, with no submissions yet. The language list (`LanguageRegistry::builtin`) and each language's own `register()` in `langs/<id>/mod.rs` stay as they are.
- **P3.0 — the `mod` line.** It is the one shared line; `.gitattributes` gives `crates/{cupel-checks,cupel-adapt,cupel-lang}/src/lib.rs` git's built-in `union` merge driver, so two branches that each add a `mod` line merge with both (checked on a scratch repository; `make fmt` restores the order if they land out of it). `crates/cupel-checks/tests/registration.rs` reads `lib.rs` through `include_str!` and fails if any code line other than its `mod` line names a registered check's module (module name = check ID with `.` and `-` as `_`), and that `register` lists checks in ID order; `crates/cupel-lang/tests/registration.rs` fails if `generic.rs` registers a producer itself. Duplicate IDs are tested by submitting a second `static.complexity` (`cupel-checks/tests/duplicate_ids.rs`) and a doubled `tool.twin` (`cupel-adapt/tests/duplicate_ids.rs`), each its own test binary so the submission reaches no other test; `cupel-adapt/tests/registration.rs` submits a probe producer from the test binary and finds it registered for its language only.
- **P3.0 — what a new check touches.** `crates/cupel-checks/src/<module>.rs` (and its `<module>/` submodules, if any) with one `inventory::submit!`; `mod <module>;` in `crates/cupel-checks/src/lib.rs`; `crates/cupel-checks/tests/<module>.rs`; fixture code, preferably in its own `crates/cupel-fixtures/repos/<check-id>/<lang>/`, which the matrix harness runs the check on instead of `repos/<lang>/` (`matrix::fixture_repo`); and `crates/cupel-fixtures/golden/<check-id>/` (`<lang>.json` per supported language, `describe.json`, `survey-<lang>.json` per language), written by `UPDATE_GOLDENS=1 make test`. Plan §2.8's "adding a check later is one module plus goldens" is now literally true, with the `mod` line and the check's own fixtures. The README's generated check table and support matrix gain its rows through `make readme`; they are the one whole-registry artifact left, and a merge conflict there is resolved by rerunning `make readme`.
- **P3.0 — what a new tool producer touches.** `crates/cupel-adapt/src/<module>.rs` with one `inventory::submit!(crate::ToolProducer { .. })`, `mod <module>;` in `crates/cupel-adapt/src/lib.rs`, and its tests and captured-output fixtures. A generic language producer is its fact module in `crates/cupel-lang/src/` with its `GenericProducer` submission and `mod` line; a language-specific one goes in that language's `register()`.
- **P3.0 — lists deleted.** In `crates/cupel-cli/tests/describe.rs`: the seven-check list and the all-supported assertion in the matrix test, the `checks × 5` cell count, the seven-check column in the Rust language test, the `norm-tree` consumer list, five hard-coded lists of the five language IDs, `languages.len() == 5`, and `languages[4]` for TypeScript; all now come from `cupel_checks::register` and `LanguageRegistry::builtin` through `support::{registered_checks, check_ids, language_ids}`. New registry-derived coverage: every registered check describes with exactly its manifest and its matrix row, every registered finding kind describes with its check and action (this replaces `readme.rs`'s `every_finding_kind_in_the_check_table_is_described`, which derived the set from `describe matrix` rather than the registry), the matrix has exactly one cell per registered check and language. Both fail when a check is registered but `describe` leaves it out (checked by making `describe` skip `tests.quality`). The `describe matrix` text snapshot is replaced by a row-per-registered-check assertion; the TypeScript language text snapshot no longer holds check rows (asserted from the matrix) or facts no language producer makes. In `crates/cupel-fixtures`: `describe/matrix.json` lost `checks` and `cells`, `describe/language-<lang>.json` lost `checks` (and keeps only language-layer producers in `facts`), `describe/languages.json` keeps only language-layer fact kinds, `describe/fact-symbols.json` lost `consumers`; each removed part is asserted from the registries in `tests/describe_goldens.rs`. `survey/<lang>.json` keeps only what no check owns; each check's results, findings, and ranking dimensions are `golden/<check-id>/survey-<lang>.json`, and the totals, severity counts, and `next_actions` are asserted against the findings (`cupel_core::next_actions`). Reassembling the slices reproduces the P2 survey goldens exactly. The matrix's stale-golden check claims `describe` and `survey-<lang>` in each registered check's directory.
- **P3.0 — goldens.** Every check's goldens are under `golden/<check-id>/`; no golden file holds two checks' content. The golden loader has no index: a name maps to `golden/<name>.json`, and `UPDATE_GOLDENS=1` writes per file.
- **P3.0 — shared test helpers.** `cupel-checks/tests/support`: `check(id)`, `recorded(check, facts, scope, config)`, `setting_error`, `score`, `hash_of_line`, `decided_facts`/`decided_scope` (the `Symbols`+`Decisions`+`NormTree` world of the complexity and CRAP tests), `imported_file`/`imported_package`; the seven test files' copies of registry lookup, plan-and-run, and settings-error matching are gone. `cupel-cli/tests/support`: `described`, `Repo::git`, `usage_argument`, `finding_fields`, `finding`, and `cli_basics.rs` uses the shared `cupel()`. `cupel-core/tests/support`: `lang`, `manifest`, `Declared` (a manifest-only check) for the scheduler, extension, selection, config-registry, and rankings tests. `cupel-lang/tests/support`: one parameterized `sites_of`, site-row `summary`/`site_row`, `site_hashes`, `relaid`, and `assert_every_replacement_parses` replace the per-language copies in the four `*_mutations.rs`; `normalize_at` is shared by Go, Rust, and ECMAScript. The fixture-repository path is `cupel_fixtures::repo_path` everywhere, so `cupel-adapt`, `cupel-lang`, and `cupel-cli` dev-depend on `cupel-fixtures`. Left in place: the small cross-crate copies between `cupel-core` and `cupel-store`/`cupel-cli`/`cupel-lang`/`cupel-checks` tests (a temporary git repository's `write`, a file walk, a `CheckResult` builder, a `ProduceCtx` builder, a `Span` builder); sharing them through `cupel-fixtures` would make `cupel-core`'s and `cupel-store`'s tests depend on the language layer.
- **P3.0 — clones** (`cupel check --depth quick` on the tree, `make dogfood DOGFOOD_SCOPE=tree`). Before (896984c): 99 groups (69 `clone.pair`, 30 `clone.cluster`), 213 similar pairs. After: 85 groups (66, 19), 168 similar pairs. `[gate.tree]` ratchets lowered to `static.clones.pairs <= 168` and `static.clones.clusters <= 19`. The registry-coverage tests iterate in helpers that return what they found, so `tests.quality` reports no `test.logic`.
- **P3.0 — crate graph.** `cupel-fixtures` depends on `cupel-adapt` (ADR-074: the matrix harness must see every producer the binary sees). `cupel-adapt`, `cupel-lang`, and `cupel-cli` dev-depend on `cupel-fixtures` (integration tests link the same library instance; the dev-dependency cycle is what `cupel-fixtures` exists for).
- **P3.0 — open items.** (1) `make dogfood` (the changed gate CI runs on branches) fails on this branch, as it did on P2's: `static.clones` reports the pre-existing groups in every file the step touched (each check module gained its submission), and `tests.weakening` reports the moved and rewritten tests (`readme.rs`'s finding-kind test now lives in `describe.rs`, the language text snapshot was rewritten). It passes once the step is on `main`. (2) Checks added before P3.0 keep their fixtures in the shared `repos/<lang>/`; their code shows up in every check's fact scope there, so editing a shared fixture still ripples into other checks' goldens. (3) The TypeScript language text snapshot aligns its facts column to the longest fact name; a producer of a longer-named fact (`mutation-outcomes`, P3.4) realigns it. (4) Language-layer producer additions still change `golden/facts/` and the `cupel-lang` tests that pin fixture contents, as before.
- **P3.0 — tests.** 1024 in all (+9 over P2's 1015); `make check` green at every commit; `make dogfood DOGFOOD_SCOPE=tree` exit 0.

---

### P4 — Schemata, daemon, budget scheduler

**Goal:** Compile-once mutation for Rust, a long-lived daemon with byte-identical output, and value-ordered scheduling under a wall-clock budget.

**Status:** not started

#### P4.1 Rust mutant schemata

**Status:** not started

- Rust schemata strategy registered with the mutation engine: rewrite each site as `if cupel_mutant_id() == N { mutated } else { original }` preserving expression types; compile once; run each mutant with `CUPEL_MUTANT=N`. Validate the schemata build compiles before running; fall back to file-copy on failure (recorded in the check result). Write **ADR-005** on Go timing.

*Tests first:* for every Rust fixture, outcomes under schemata == outcomes under file-copy (the design's schemata validation test); a fixture that defeats the type-preservation rule triggers fallback cleanly.

#### P4.2 Daemon (`serve`)

**Status:** not started

- `cupel-serve`: holds parsed trees (content-hash keyed), store connection, coverage map, last plan; `notify` watcher for incremental reparse and re-hash; Unix socket at `.cupel/serve.sock`; CLI auto-connects when present, `CUPEL_NO_DAEMON=1` bypasses. Write **ADR-009**.
- Parity test: every fixture run cold and via daemon is byte-identical after redaction.

#### P4.3 Budget scheduler

**Status:** not started

- Cost from store history (p50 of last 10 runs of the task shape) else manifest default; value = `tier_weight × novelty × (changed ? 2 : 1)`; greedy knapsack with dependency closure; skipped tasks reported with `reason: budget` and `est_ms`. `--jobs` concurrency cap; CPU-bound on rayon, process-spawning on tokio.

*Tests first:* scheduler unit tests on synthetic DAGs (budget respected, dependencies honored, highest value chosen, deterministic order given a seed); `--budget 1s` on a fixture skips mutation and says so.

#### P4 Definition of Done

- Schemata parity holds for all Rust fixtures; warm daemon `check --depth quick` < 1 s on the Cupel repo.
- `--budget` never exceeds its wall clock by more than one task's timeout.
- README: `serve`, `--budget`, performance characteristics.

#### P4 Notes

_(none yet)_

---

### P5 — Plugin protocol, oracles, generative checks

**Goal:** Checks outside the binary, verified oracles, and tier-4 generative evidence.

**Status:** not started

#### P5.1 Plugin host and reference plugins

**Status:** not started

- `cupel-plugin`: `Manifest` (`kind: check | producer`), `RunRequest`, event types including `fact`; discovery via `[plugins]` and `cupel-check-*`/`cupel-produce-*` on `$PATH` (`--no-path-plugins`); `describe`/`plan`/`run` subprocess protocol over stdio per design §8.6; budget enforcement (SIGTERM then SIGKILL); manifest schema validation; undeclared finding kinds rejected; emitted facts validated against the fact schema at the declared version; non-zero exit without `result`/`fact` → `status: error`.
- Check plugins receive their declared facts as JSON files and enter the `CheckRegistry`; producer plugins enter the `ProducerRegistry` and are selected by the same rules as native producers, so they appear in `describe matrix`.
- Reference plugins in Rust, Python, and TypeScript under `crates/cupel-fixtures/plugins/`: each language has one trivial check plugin and one trivial producer plugin (e.g. a `CoverageMap` producer that reads a committed LCOV file), used by a protocol conformance suite. Property tests on request/event/fact round-trips.

*Tests first:* a producer plugin registered for a fixture language flips a `(check, lang)` pair from `unsupported` to `supported` in `describe matrix` and the matrix test then demands its golden.

#### P5.2 `oracle` verb

**Status:** not started

- `add NAME --kind ref|command|spec …`, `list`, `remove`, `verify` (smoke comparison; records `verified_at` + hash); `oracles` table. Results against unverified oracles capped at tier 3 with `warn: oracle.unverified`.

#### P5.3 `gen.differential`

**Status:** not started

- Old-vs-new (ref oracle) or impl-vs-command on generated inputs; input generators per language; minimized divergence with both outputs in evidence.

#### P5.4 `gen.property`

**Status:** not started

- Wrap Hypothesis, proptest/quickcheck, fast-check with seed and case budget; minimized counterexamples; `--scaffold` emits property stubs from signatures (written only with `--write`). Kinds `property.falsified`, `property.missing`.

#### P5.5 `gen.metamorphic`

**Status:** not started

- Relations declared in config, evaluated on generated inputs. Kind `metamorphic.violated`.

#### P5 Definition of Done

- Protocol conformance suite green for all six reference plugins (three check, three producer).
- Fixtures with a known divergence / falsifiable property / violated relation produce the exact expected finding.
- README: plugins, oracles, `gen.*`.

#### P5 Notes

_(none yet)_

---

### P6 — Wrapped tiers, SARIF, additional languages

**Status:** not started

#### P6.1 SARIF renderer

**Status:** not started

- SARIF 2.1.0 per §7.3; validate against the official schema in a test.

#### P6.2 `static.analyze`, `static.api-surface`, `perf.footprint`

**Status:** not started

- `static.api-surface`: `needs = [ApiSurface]` for base and head — a pure diff, no new facts. `static.analyze`: `AnalyzerReport` producers wrapping each ecosystem's type checker and linters (SARIF-normalized). `perf.footprint`: a `Footprint` fact (binary/bundle size, dependency count, licenses, vulns) — not in the v1 catalog, so **write an ADR** before adding it.

#### P6.3 `conc.race`, `perf.bench`, `gen.fuzz`

**Status:** not started

- Producers: `RaceReports` (Go `-race`, TSan, Miri), `BenchSamples` (criterion, benchstat, hyperfine, pytest-benchmark) with statistical comparison against baseline done in the check, `FuzzResults` (cargo-fuzz, Go native fuzz, atheris, jazzer.js). Each check is one module over its fact; each new tool is one producer.

#### P6.4 Java and Clojure

**Status:** not started

- Each added by following the design §8.4.4 checklist verbatim, as the proof that the extension model works: new `langs/<id>/` directory with producers for every syntactic fact, fixture repo with full `inventory.toml`, toolchain table row (JUnit/Maven/Gradle + JaCoCo; `clojure.test` + Cloverage), conformance green, every golden the matrix test demands. Java `MutationOutcomes` comes from a PIT producer plugin — `tests.mutation` is untouched. Go schemata per ADR-005 if deferred from P4.
- Record in Notes the exact set of files touched outside `langs/`, `cupel-fixtures/`, `cupel-adapt`'s toolchain table, and the PIT plugin. The target is zero; anything else is an extension-model defect to fix.

#### P6.5 Extension drill

**Status:** not started

- Add one genuinely new check over existing facts (candidate: `tests.assertion-density` over `TestFrames` and `Symbols`, or `static.fan-out` over `ModuleGraph`) and record in Notes the files touched. The target is: one module in `cupel-checks`, one line registering it, goldens. Anything else is an extension-model defect to fix before P7.

#### P6 Definition of Done

- Every check in §5.1, §5.3 (except `gen.symbolic`), §5.4 `conc.race`, §5.6 has goldens for every `supported` language; seven languages pass the conformance suites; `describe matrix` has no `unsupported` cell that lacks a one-line reason in Notes.
- P6.4 and P6.5 Notes each record zero files touched outside their expected set.
- README: full language and check matrix.

#### P6 Notes

_(none yet)_

---

### P7 — Formal, distributed, MCP

**Status:** not started

#### P7.1 `spec.contracts` · P7.2 `spec.trace` (ADR-007) · P7.3 `conc.linearizability` · P7.4 `conc.dst` · P7.5 `conc.model-check` and `spec.prove` · P7.6 `meta.traceability` (ADR-008) and `meta.oracle-independence` · P7.7 `serve --mcp`

**Status:** not started (each sub-step gets its own status line when P7 begins)

Each wrapped tool is a producer: parser tested on committed real output; integration test runs the real tool when installed, otherwise reports skipped visibly. The P7 facts (`Traces`, `Histories`, `ProofResults`, `ContractViolations`) are named in design §8.3.2 and so need no ADR to add, but each needs its conformance inventory and schema before its first producer.

#### P7 Definition of Done

- Every check in the catalog (§5) is implemented or has an ADR explaining deferral.
- MCP tools return the envelope; events stream as progress notifications; parity with CLI output.

#### P7 Notes

_(none yet)_

---

### P8 — Distribution and agent surface

**Status:** not started

#### P8.1 Cross-platform release builds

**Status:** not started

- Linux x86_64/aarch64 (musl, static), macOS arm64/x86_64, Windows x86_64 best-effort; `cargo-zigbuild`/`cross` in CI; GitHub Releases on tag.

#### P8.2 Installers

**Status:** not started

- `cargo-binstall` metadata; install script; Homebrew tap; thin `npx cupel` and `pipx` wrappers. Each installer tested in CI on a clean image.

#### P8.3 `describe --agent` and generated README sections

**Status:** not started

- `describe --agent`: ~60 lines of markdown for `CLAUDE.md`/`AGENTS.md`.
- README verb/check/exit-code tables and the **support matrix** (checks × languages) generated from `describe --all` and `describe matrix` by a `make readme` target; a test fails if the README is stale.

#### P8 Definition of Done

- A fresh machine can install Cupel by every advertised route and run the quick start.
- README is final-form per §2.7 and passes the staleness test.

#### P8 Notes

_(none yet)_

---

### P9 — Dogfood: use Cupel to improve Cupel

**Goal:** Cupel grades its own codebase at full depth and the agent acts on every finding, demonstrating the agent loop the tool exists for.

**Status:** not started

#### P9.1 Survey

**Status:** not started

- `cupel survey --depth deep` on the repo. Record the full `rankings` section in Notes verbatim (it is the baseline the rest of P9 improves on).

#### P9.2 Act on findings

**Status:** not started

- Work through `next_actions` in priority order. For each: fix (not suppress) unless the finding is a true false positive, in which case the false positive is a Cupel bug — fix the check, add the case to the fixtures, and note it. Record in Notes every check that produced a false positive or missed something obvious.

#### P9.3 Raise the gate

**Status:** not started

- Tighten `[gate.changed]` and `[gate.tree]` in Cupel's `cupel.toml` to the strictest values the codebase can sustain; `tests.mutation.score ≥ 0.80` is the floor. Record final scores in Notes.

#### P9.4 Performance baseline

**Status:** not started

- `perf.bench` against the fixture tree with a stored baseline; CI fails on > 10% regression at p95 (§11).

#### P9.5 Agent integration

**Status:** not started

- `cupel describe --agent` output added to this repo's agent instructions so every future change to Cupel is graded by Cupel.

#### P9 Definition of Done

- `cupel check --depth deep --gate` exits 0 on the tree.
- Every finding Cupel raised on itself was fixed, or is suppressed with a reason that names a real limitation.
- Notes contain: initial survey, list of real defects Cupel found in itself, list of Cupel bugs the dogfood exposed, final scores.

#### P9 Notes

_(none yet)_

---

## 4. Cross-phase checklist

Run this at the end of every step, before committing:

- [ ] Failing test was written and observed failing before implementation
- [ ] `make check` green (fmt, clippy `-D warnings`, tests, docs, clean tree)
- [ ] No `todo!()`, `unimplemented!()`, commented-out code, or stray files
- [ ] Every new file has an `// ABOUTME:` header
- [ ] `describe` reflects any new verb, flag, check, finding kind, fact, or producer; `describe matrix` is what you expect
- [ ] No language name appears in `cupel-checks`; no check name appears in `cupel-lang`, `cupel-adapt`, or `cupel-mutate`
- [ ] Goldens updated deliberately (`UPDATE_GOLDENS=1`) and the diff reviewed, never blindly
- [ ] Status line and Notes in this document updated
- [ ] ADR written if a decision was made
- [ ] README reflects what exists (at phase boundaries at minimum)
- [ ] Committed
