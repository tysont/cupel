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

Tests must never only validate mocked behavior. Adapters that wrap external tools are tested against **real** tool output captured as fixtures (parsers) *and* against the real tool when it is present on the machine (integration, skipped with a visible `SKIPPED: <tool> not installed` reason, never silently).

Test output must be pristine: a passing run prints no warnings, no stray `println!`, no leftover files.

### 1.4 Test taxonomy

| Kind | Where | Tool | What it proves |
|---|---|---|---|
| Unit | `src/**` `#[cfg(test)]` | `cargo test` | Pure logic: hashing, scoring, normalization, config precedence, scheduler ordering |
| Property | `src/**` `#[cfg(test)]` | `proptest` | Invariants: structural hash is rename/reformat-invariant; schema round-trips; finding IDs stable under relocation |
| Snapshot | `tests/` and `src/**` | `insta` | JSON Schema output, `describe` output, text renderer output |
| CLI integration | `crates/cupel-cli/tests/` | `assert_cmd` + `predicates` | Verbs, flags, exit codes, stdout/stderr discipline, TTY vs pipe format |
| Golden envelope | `crates/cupel-fixtures/` | custom harness (§P0.6) | Each check against each language fixture produces the expected envelope modulo volatile fields |
| External-tool integration | `crates/cupel-adapt/tests/` | `assert_cmd`, real tools | Adapters drive real `cargo test`, `pytest`, `vitest`, etc. when installed |
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
│   ├── cupel-core/            # Envelope/Finding/CheckResult types, schemas, scope, scheduler, config, Check trait
│   ├── cupel-lang/            # LanguageAdapter trait, structural hashing, parse cache
│   │   └── src/langs/<id>/    # one dir per language: mod.rs + queries/*.scm + tables.toml (design §8.3.2)
│   ├── cupel-store/           # SQLite fact store, migrations, queries
│   ├── cupel-checks/          # native checks, one module per check (see ADR-001)
│   ├── cupel-adapt/           # coverage/runner/bench adapters, one module per tool
│   ├── cupel-plugin/          # plugin protocol types + external-process host (P5)
│   ├── cupel-serve/           # daemon, socket protocol, watcher, MCP (P4, P7)
│   └── cupel-fixtures/        # fixture repos + golden envelopes + harness (dev-dependency)
│       ├── conformance/constructs.toml   # shared construct inventory every language must cover (design §8.3.4)
│       ├── repos/<lang>/...   # known-bad source trees, each with inventory.toml mapping construct IDs to code
│       └── golden/<check>/<lang>.json
└── docs/
    ├── CUPEL_TECHNICAL_DESIGN.md
    ├── CUPEL_IMPLEMENTATION_PLAN.md
    └── adr/
        └── CUPEL_DECISION_001.md ...
```

`cupel-checks` as a single crate (modules per check) is a deliberate deviation from the design's `cupel-check-*` layout; see ADR-001 (§2.5). Split a check out into its own crate only when it acquires heavy, check-specific dependencies.

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
| `make lint` | `cargo fmt --check` + `cargo clippy --workspace --all-targets -- -D warnings` |
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

| # | Decision | Write at | Guidance |
|---|---|---|---|
| 001 | Crate granularity: `cupel-checks` single crate vs `cupel-check-*` per check | P0.1 | Plan prescribes single crate; ADR records the reasoning and the split criterion |
| 002 | Git access: shell out to `git` vs `gix`/`git2` | P2.1 | Shelling out keeps the binary small and matches the user's git; a library gives speed in the daemon. Measure before choosing |
| 003 | Golden envelope redaction and comparison strategy | P0.6 | Which fields are volatile, how goldens are updated (`UPDATE_GOLDENS=1`), how diffs are reported |
| 004 | Equivalent-mutant handling in v1 (design open question 2) | P3.4 | Design leans toward emitting `mutant.equivalent?` with `confidence ≤ 0.3`, `severity: info` |
| 005 | Go schemata timing: P4 vs P6 (design open question 3) | P4.1 | Go adapter and fixtures exist from P1, so the generic-inference edge cases can be measured before deciding |
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

### 2.8 Language parity rule

Cupel launches with five adapters — `javascript`, `typescript`, `rust`, `go`, `python` — and they are peers. Throughout this plan, "every language" means all five.

- Every P1 step is done for all five adapters before the step is marked complete. The trait is shaped by five implementations, not one.
- Every check golden exists for every language the check applies to. A check that does not apply to a language says so through `Capabilities` and is covered by a test asserting the precise `skipped` reason.
- The conformance suite (design §8.3.4) runs as part of `make test`. A construct ID with no mapping in a language's `inventory.toml` is a build failure, not a warning.
- Adding a language later (Java and Clojure in P6) follows the design §8.3.4 checklist exactly. If the checklist turns out to require a change in `cupel-core`, a check, or the CLI, stop: write an ADR, fix the extension point, then continue.

---

## 3. Phases

---

### P0 — Platform skeleton

**Goal:** A `cupel` binary with the complete output contract, CLI surface, configuration system, `describe`/`init`/`doctor` verbs, and a fixture/golden harness — but no language analysis yet. Everything later plugs into this.

**Status:** not started

#### P0.1 Workspace, automation, CI

**Status:** not started

- Cargo workspace with `cupel-cli` and `cupel-core` crates; `rust-toolchain.toml`; `[workspace.lints]` enabling `clippy::all`, `clippy::pedantic` selectively, `unsafe_code = "forbid"` (relax per-crate only via ADR).
- `Makefile` per §2.3. `.github/workflows/ci.yml` running `make check` on Linux and macOS.
- `.gitignore` per §2.6.
- `cupel --version` prints `cupel <semver>`; `cupel` with no args prints usage to stderr and exits 2.
- Write **ADR-001**.

*Tests first:* `tests/cli_basics.rs` — version string shape, no-args exit code 2, unknown verb exit 2.

#### P0.2 Core types and schemas

**Status:** not started

In `cupel-core`, define the output contract exactly as design §7 specifies, deriving `Serialize`, `Deserialize`, `JsonSchema`:

- `Envelope`, `RunInfo`, `GitInfo`, `ScopeInfo`, `ToolchainInfo`, `ConfigEcho`, `BaselineRef`
- `Status` (`pass | fail | partial | error | plan`), `ExitCode` (0–6 with names from §6.6)
- `CheckResult`, `Score`, `Delta`
- `Finding`, `Severity`, `Symbol`, `Location`, `Action`, `ActionKind` (closed enum from §7.2), `Evidence` (tagged per kind)
- `Summary`, `GateResult`, `GateRule`, `NextAction`
- `Event` (all `ev` variants from §7.4)
- `FindingId::compute(check, kind, symbol_hash, site_hash)` = `fnd_` + first 24 bits of blake3 as hex (§7.2)
- `RunId` (ULID, `run_` prefix); `SymbolHash` (`sh_` prefix)

Schemas are generated by `schemars` and exposed via `cupel_core::schema::{run, finding, events, config, plugin}()`.

*Tests first:* serde round-trip for every type; `insta` snapshots of every schema; unit tests for `FindingId` (known vector, 24-bit truncation, prefix); property test that `FindingId` ignores line numbers (it takes only structural inputs by construction — test that the API makes it impossible to pass a line).

#### P0.3 CLI framework and `describe`

**Status:** not started

- `clap` derive with every global flag from §6.3 on a shared struct; every verb from §6.2 declared (unimplemented verbs return exit 2 with a finding `kind: cli.not-implemented` and a `detail` naming the phase — remove this as verbs land).
- Format auto-detection: JSON when stdout is not a terminal, text when it is; `--format` overrides; `CUPEL_FORMAT` env respected.
- Exit-code mapping: every verb returns `Result<Envelope, CupelError>`; the binary converts to envelope + exit code. **Every** error path produces a valid envelope on stdout (usage → exit 2 with a finding naming the fix; internal → exit 6 with bug-report payload). stderr is only for `--verbose`/`--quiet`-controlled diagnostics and `--events stderr`.
- Self-description registry: a single data structure describing verbs, flags (type, default, enum values), exit codes, and schema URLs. **Both** clap help and `describe` render from it so they cannot drift.
- `cupel describe`, `cupel describe <verb>`, `cupel describe schema <name>`, `cupel describe --all`, `cupel <verb> --describe`. `describe <check-id>` and `describe finding <kind>` come online as checks land (P2+). `describe --agent` is P8.3.
- Every described entity carries `stability`.

*Tests first:* `tests/describe.rs` snapshots; `tests/format.rs` (pipe → JSON, `--format text` → text, invalid format → exit 2 envelope); `tests/exit_codes.rs` (each code reachable via a deliberate trigger); a test asserting clap's `--help` output mentions every flag the registry declares and vice versa.

#### P0.4 Configuration

**Status:** not started

- `cupel.toml` model per §6.7 with `serde` + `toml`; unknown keys at any level are usage errors (exit 2) listing valid keys.
- Discovery: `--config PATH`, else nearest `cupel.toml` walking up from `--cwd` (default: current dir); `--cwd` establishes repo root — **never** rely on process cwd elsewhere.
- Precedence: flags > `CUPEL_*` env > `--profile` > `cupel.toml` > defaults. Effective config and its sources echoed under `run.config`.
- Gate rule key validation (`<check-id>.<metric>` or `<check-id>`; operators `>= > <= < == none no-breaking`) — parsing only; evaluation is P2.9. Unknown check IDs error at parse time once the check registry exists (P2.1); until then, validate syntax only.

*Tests first:* precedence matrix as a table-driven unit test; unknown-key error message snapshot; walking-up discovery with `tempfile`; profile merge.

#### P0.5 Renderers: JSON, JSONL, text

**Status:** not started

- `json`: pretty, stable key order (serde struct order), trailing newline.
- `jsonl`: header line (envelope minus `findings`), then one finding per line.
- `text`: per §7.3 — status line, gate result, findings grouped by severity then check, each with location, title, action summary, repro; `next_actions` last; evidence summarized to one line. `--no-color` and non-TTY disable color. No information in JSON is omitted except evidence bodies.
- `--limit`/`--offset` paging with `truncated` and `total_findings` always set.

*Tests first:* snapshot each renderer against a hand-built envelope containing every field; paging arithmetic unit tests; property test that `jsonl` header + lines reassemble into the `json` envelope.

#### P0.6 Fixture and golden-envelope harness

**Status:** not started

- `crates/cupel-fixtures` with `repos/` (populated per language from P1), `golden/`, and a library offering `run_cupel(args, fixture) -> Envelope` and `assert_golden(name, envelope)`.
- Redaction of volatile fields (`run.id`, `run.started`, all `duration_ms`, `toolchain` versions, absolute paths, `git.sha`) before comparison. `UPDATE_GOLDENS=1 make test` rewrites goldens; CI never sets it. Golden diffs are printed as unified diffs.
- Write **ADR-003**.

*Tests first:* the harness tests itself — a trivial `describe` envelope round-trips through redaction; an intentionally changed field produces a readable diff.

#### P0.7 Run infrastructure: `.cupel/`, run IDs, events, timeout, dry-run plumbing

**Status:** not started

- `.cupel/` directory conventions: `cupel.db` (P2.2), `runs/<id>.json`, `cache/`, `work/<run>/`, `.gitignore` written by `init`. Nothing is written outside `.cupel/` without `--write`; a central `Writer` type enforces this and is the only way to write files.
- `--events PATH|stderr` JSONL sink; stdout reserved for the envelope.
- `--timeout DURATION` (parse `10s`, `5m`, `1h`): cooperative cancellation token; on expiry, outstanding work is cancelled, `status: partial`, exit 5, partial results included.
- `--dry-run` produces a `plan` envelope (§6.8) with `would_run`, `would_skip`, `would_install`, `would_write`; for writing verbs `would_write` carries unified diffs.
- `--seed` recorded in `run.seed` (default: random, always echoed).

*Tests first:* `Writer` refuses paths outside `.cupel/` without `--write` (unit); events file contains `plan` … `run.end` for a trivial verb; `--timeout 1ms` on a verb with an injected slow task yields exit 5 and `partial`.

#### P0.8 `init` and `doctor`

**Status:** not started

- `init`: detects project roots and languages (Cargo workspace, `go.work`/`go.mod`, npm/pnpm/yarn workspaces, uv/poetry/pip, Maven/Gradle, `deps.edn`) and the default test/coverage/bench commands per language (the detection table lives in `cupel-lang` once P1 lands; in P0 it is a standalone detection module that P1.7 moves). Writes `cupel.toml` and `.cupel/.gitignore` only with `--write`; `--dry-run` shows the file as a diff; refuses to overwrite without `--force`.
- `doctor`: binary version vs `required_version`; config validity; write access to `.cupel/`; one finding per problem with `action.kind: configure | install_tool`; exit 3 if anything required is missing, else 0. Adapter tool checks are added as adapters land (P3+). `--fix --install` is wired but has nothing to install until P3.

*Tests first:* `init --dry-run` golden per fixture repo type (minimal Cargo, npm, pyproject, Go module fixtures under `cupel-fixtures/repos/detect/`); `init` without `--write` writes nothing (assert tree unchanged); `--force` semantics; `doctor` on a repo with bad `required_version` → exit 3.

#### P0 Definition of Done

- All P0.1–P0.8 steps complete.
- `cupel describe --all` emits one JSON document covering verbs, flags, exit codes, schemas.
- Every verb from §6.2 is reachable; unimplemented ones return exit 2 with `cli.not-implemented`.
- `make check` green on Linux and macOS CI.
- README describes: purpose, install from source, `describe`, `init`, `doctor`, output contract, exit codes.

#### P0 Notes

_(none yet)_

---

### P1 — Language layer

**Goal:** `cupel-lang` with the `LanguageAdapter` trait (design §8.3) fully implemented for JavaScript, TypeScript, Rust, Go, and Python; the declarative `LanguageSpec` + `Capabilities` core; structural hashing; a content-hash parse cache; a parallel tree walk; and the conformance suite that makes the trait an enforceable contract.

**Status:** not started

Each step below is done for all five adapters before moving on (§2.8). Fixture repos under `cupel-fixtures/repos/{javascript,typescript,rust,go,python}/` are created in P1.1 and grow with each step; every fixture file is paired with an expected-inventory JSON, and every construct the step introduces is added to `conformance/constructs.toml` and mapped in each language's `inventory.toml`.

#### P1.1 Trait, spec, capabilities, grammars, walk, cache, conformance skeleton

**Status:** not started

- `LanguageAdapter` trait exactly as design §8.3.1 with query-driven default methods reading `LanguageSpec`; `LangId`, `LangFamily`, `Capabilities`; registry of adapters by extension; `grammar(path)` selecting among an adapter's grammars (`javascript`: js/jsx; `typescript`: typescript/tsx).
- `langs/<id>/` directory per language with `mod.rs`, `queries/`, `tables.toml`; queries embedded with `include_str!`; a test compiles every `.scm` against its grammar.
- `javascript` and `typescript` share one implementation module with two `LangId`s and the `ecma` family.
- Parallel walk with `ignore` (gitignore-aware), rayon parse; parse cache keyed by blake3 of file content.
- `cupel-fixtures/conformance/constructs.toml` created; each language's `inventory.toml` created; the conformance test harness iterates adapters × constructs and fails on any unmapped construct.
- `cupel describe language <id>` and `describe languages` render identity and capabilities.

*Tests first:* every `.scm` compiles; walk returns exactly the fixture's expected file list (gitignored files excluded); cache hit rate is 100% on second walk; parse errors produce a `lang.parse-error` finding, never a panic; `.tsx` and `.jsx` files parse with the right grammar; conformance harness fails on a deliberately unmapped construct.

#### P1.2 Symbols

**Status:** not started

- `symbols()` → functions, methods, nested functions, closures assigned to names, with namespace (`module::Type::method`), visibility, span, body node. Language specifics: Go method receivers and package-level namespace; Rust `impl` blocks and trait impls; Python nested `def` and class methods; JS/TS arrow functions assigned to `const`, class methods, exported function expressions.

*Tests first:* expected-inventory JSON per fixture file (name, ns, visibility, start/end line); conformance constructs `sym.*`.

#### P1.3 Decision points

**Status:** not started

- `decision_points()` for cyclomatic and cognitive (nesting-weighted) complexity, following crapper's per-language tables (design §3), expressed as `decisions.scm` captures plus weights in `tables.toml`.

*Tests first:* functions with known CC/cognitive values; the weights table is the single source — one fixture function per construct per language; conformance constructs `cc.*` (with `not-applicable` for e.g. Go ternary, Rust `switch`).

#### P1.4 Normalization and structural hash

**Status:** not started

- `normalize()` → `NormTree`: locals, fields, literals, keywords → generic markers; call names, operators, tree shape preserved (dryer's rules, design §3). Normalize rules live in `tables.toml`; adapters override only for constructs the rules cannot express (Python `self`, Rust macros, Go receivers).
- `NormTree` serialization is language-independent so `ecma`-family symbols compare across JS and TS.
- `Symbol.hash = blake3(lang ∥ serialize(NormTree))`, `sh_` prefix.

*Tests first:* property tests — hash invariant under: rename of locals/params, reformatting (whitespace/newlines), function rename, file move; hash **changes** under: operator swap, added branch, changed call target. Fixture pairs for each language. A JS function and its TS-annotated twin normalize to equal `NormTree`s.

#### P1.5 Mutation sites

**Status:** not started

- `mutation_sites()` with operator set `default` (boolean flips, relational/equality swaps, `+/-`, `*//`, `&&/||`, `0/1`) and `extended` (statement deletion, return-value replacement, constant replacement, boundary ±1, negation), each site carrying a `site_hash` (structural position within the symbol, not line). Operators and replacements are `mutations.scm` captures plus `tables.toml` entries (e.g. Python `and/or`, Go `&&/||`, Rust `&&/||`; Python `True/False`, others `true/false`).

*Tests first:* expected-sites JSON per fixture function; `site_hash` unchanged when the function is moved or reformatted (property); conformance constructs `mut.*`; every generated replacement re-parses without a syntax error in its language (property over the fixture corpus).

#### P1.6 Test frames

**Status:** not started

- `test_frames()` → test cases with their assertions, mocks, skip markers, and setup blocks for each language's dominant frameworks: vitest/jest/mocha/`node:test` (JS/TS), Rust `#[test]`/`#[tokio::test]`, Go `testing.T`/testify, pytest/unittest. Weak-assertion patterns per framework in `tables.toml`.
- `is_test_file()` and `test_files_for()` conventions: `*.test.ts`/`__tests__/`, `#[cfg(test)]`/`tests/`, `*_test.go`, `test_*.py`/`tests/`.

*Tests first:* expected-frames JSON per fixture test file; conformance constructs `test.*`; `test_files_for(src)` returns the fixture's actual test file for every source file that has one.

#### P1.7 Imports, module resolution, overlay dependencies

**Status:** not started

- `imports()` → edges with raw specifiers via `imports.scm` (ESM/CJS `import`/`require`, Rust `use`/`mod`, Go `import`, Python `import`/`from … import`).
- `resolve_import()` → `Resolution::{File(path), External(name), Unresolved}`: Node resolution with `package.json` `exports`/`main` and `tsconfig` `paths`; Rust `mod` tree and workspace members; Go module path → directory via `go.mod`; Python packages via `__init__.py` and project roots.
- `overlay_deps()` → files a mutant overlay must copy alongside the mutated file (Node relative imports; empty for Rust/Go/Python).

*Tests first:* fixture per language with a known import graph (including one external and one unresolvable specifier); resolved graph equals the expected edge list exactly; a TS fixture with `paths` aliases resolves correctly.

#### P1.8 Public API and dead-code roots

**Status:** not started

- `public_api()` → exported/`pub`/capitalized/`__all__` items with normalized signatures via `api.scm`.
- `roots()` → symbols live by convention: `main`, exported items, test functions, framework hooks (`init()`, `__init__`, default exports, `#[no_mangle]`, Go `TestMain`).

*Tests first:* expected API JSON per fixture; a fixture change that removes a public parameter shows in the API diff; every test function in every fixture is reported as a root.

#### P1.9 Project detection and toolchain table

**Status:** not started

- Move P0.8's detection into `project()` on each adapter; `ProjectInfo` (build file, roots, default test/coverage/bench commands, dirs to skip). JS vs TS decided by `tsconfig.json` presence.
- `cupel-adapt` toolchain table keyed by `LangId` per design §8.6 (runner detection rules, coverage producers/formats, bench, property/fuzz/race tools); adapter implementations themselves land in P3. `init`, `doctor`, and `describe language` read the table.

*Tests first:* P0.8's `init` goldens continue to pass unchanged; a monorepo fixture with a Rust crate, a Go module, a TS package, and a Python package yields four roots with the right languages; `describe language go` snapshot.

#### P1 Definition of Done

- All five adapters implement the full trait (except `schemata`, which returns `None` until P4.1) and the conformance suite is green with zero unmapped constructs.
- Each adapter's `mod.rs` is thin: any analysis method override is justified by a comment naming the construct queries could not express.
- `cupel describe languages` lists five languages with capabilities and stability.
- Parse of a 100k-LOC synthetic tree completes in < 2 s cold on the dev machine (record the number in Notes).
- README lists supported languages with a capability matrix.

#### P1 Notes

_(none yet)_

---

### P2 — Static checks (first vertical slices)

**Goal:** The `check` and `survey` verbs run real checks over a resolved scope, persist to the fact store, compare to baselines, honor suppressions, evaluate gates, and explain findings. Each check is a complete vertical slice.

**Status:** not started

Each check step (P2.3–P2.8) follows the same recipe and is not done until all of it is:

1. Fixture code exhibiting each finding kind (and clean code that must produce none) in all applicable languages.
2. Golden envelopes written **first** (hand-authored from the design's finding spec) and failing.
3. `Manifest` (id, tier, cost, languages, prereqs, flags, finding kinds → action kinds, stability, docs).
4. `plan()` with cost estimate; `run()` emitting findings through the `EventSink`.
5. `describe <check-id>` and `describe finding <kind>` populated from the manifest.
6. `explain <finding-id>` renders the check's evidence (source excerpt, pair side-by-side, etc.).
7. Text renderer handles the new evidence shape in one line.
8. Goldens pass; `make check` green; README check table updated.

#### P2.1 `Check` trait, registry, scope, scheduler v1, `check` verb with `static.complexity`

**Status:** not started

The first end-to-end slice. Scope: make `cupel check --paths src/ --only static.complexity` produce correct `complexity.high`, `nesting.deep`, `params.many`, `function.long` findings with correct IDs, actions (`reduce_complexity`, `refactor_extract`), and repros.

- `Check` trait per §8.4; `Ctx`, `Plan`, `EventSink`; check registry by ID and group.
- Scope resolution: `--paths` (globs, gitignore-aware), `--changed` (merge-base with `default_branch`, includes working tree), `--base/--head`, `--symbols` (ns::name or `sh_` hash); intersection semantics; empty `--changed` → `pass` envelope with `scope.symbols: 0`. Write **ADR-002** (git access).
- Check selection by depth (§5.8), `--only/--skip` (IDs and groups), `--tier N+`, language applicability.
- Scheduler v1: dependency-ordered sequential execution; rayon within a check; no budget yet (P4.3).
- `static.complexity` as the first check; thresholds configurable under `[checks."static.complexity"]`.

*Tests first:* scope resolution matrix (unit, with a `tempfile` git repo); `--only`/`--skip` group expansion; golden envelopes for `static.complexity` on all five fixture languages; `scope.languages` lists exactly the adapters that saw files; `--changed` on a clean tree → exit 0, `scope.symbols: 0`.

#### P2.2 Fact store, `explain`, baselines, `suppress`

**Status:** not started

- `cupel-store`: SQLite WAL at `.cupel/cupel.db`; migrations; tables `symbols`, `metrics`, `runs`, `findings`, `baselines`, `suppressions` (remaining tables as their phases land). Single-writer lock.
- Every `check`/`survey` run persisted (envelope + findings + metrics).
- `explain <finding-id>` from the store; `explain <check-id>` returns last run's full per-symbol result. Never re-runs.
- `baseline set [--from run:ID] --write`, `show`, `diff [--against …]`, `export FILE`, `import FILE`, `--name`.
- `--against baseline|REF|run:ID`; `new_since_baseline` on findings; `summary.new/resolved`; `delta` on check results.
- `suppress <id> --reason --until REF|DATE` → `.cupel/suppressions.toml` (committable, so `.cupel/.gitignore` excludes it); suppressed findings appear with `suppressed: true`, `severity: info`; expired suppressions emit `suppression.expired`.

*Tests first:* store round-trip; migration from empty; `explain` on unknown ID → exit 2 envelope; baseline diff golden (run A, modify fixture, run B, `diff` shows exactly the new/resolved set); suppression lifecycle including expiry by date.

#### P2.3 `static.crap` (CC-only mode)

**Status:** not started

- `CC² × (1 − coverage)³ + CC` with `--no-coverage` forcing coverage = 0; without a coverage adapter (P3.2) the check reports `skipped: "no coverage; use --no-coverage"` rather than silently computing CC-only. Kinds `crap.high` (≥ 30), `crap.watch` (5–30).

#### P2.4 `static.clones`

**Status:** not started

- Per-symbol subtree fingerprints from `NormTree`; MinHash signatures → LSH bands → pairwise Jaccard within buckets only. Defaults `threshold 0.82`, `min_lines 4`, `min_nodes 20`; same-family only (JS↔TS pairs are reported; Go↔Python never). Kinds `clone.pair` (both spans in evidence), `clone.cluster`.

*Additional tests:* property — MinHash/LSH never misses a pair whose true Jaccard ≥ threshold + 0.05 on generated fingerprint sets (recall bound); performance — 10k synthetic symbols in < 1 s.

#### P2.5 `static.deps`

**Status:** not started

- Module graph from `imports()` + `resolve_import()` (P1.7); external and unresolved edges excluded from coupling but counted in evidence; afferent/efferent coupling, instability, abstractness, distance from main sequence; Tarjan SCC for cycles; layering rules from `[checks."static.deps".layers]`. Kinds `dep.cycle`, `dep.layer-violation`, `dep.unstable-abstract`.

#### P2.6 `tests.quality`

**Status:** not started

- Over `test_frames()`: assertion density, weak assertions (`assert!(true)`, `assertTrue(x is not None)`-style), mock density, logic in tests, duplicated setup. Kinds per §5.2; each finding carries SCRAP-style remediation mode and actionability class in evidence.

#### P2.7 `tests.weakening`

**Status:** not started

- Diff of `test_frames()` between base and head: removed assertions, skipped/disabled tests, loosened matchers, rewritten snapshots, deleted tests. Default `severity: fail`. Requires a range scope; on a tree scope it is `skipped: "requires --changed or --base"`.

*Additional tests:* a fixture git history (constructed in a `tempfile` repo during the test) with one commit per weakening kind.

#### P2.8 `static.dead-code`

**Status:** not started

- Unreferenced symbols and exports using the symbol table, the resolved import graph, and `roots()` (P1.8) as the live set; conservative — anything reachable from a root or exported from a library root is not dead. Kinds `dead.symbol`, `dead.export`. `confidence` reflects uncertainty (dynamic dispatch, reflection, `getattr`).

#### P2.9 `survey`, gates, `next_actions`

**Status:** not started

- `survey`: same scope/depth; no gate; adds `rankings` (worst CRAP, largest clone clusters, most-coupled modules, dependency cycles; mutation/flaky dimensions join in P3).
- Gate evaluation per §6.7: `[gate.changed]` for range scopes, `[gate.tree]` otherwise; `--gate/--no-gate`; rule results in `summary.gate`; exit 1 on fail.
- `next_actions`: ranked by `severity × tier × new_since_baseline`, de-duplicated by action kind + location; `rerun` actions for skipped checks with their command.

*Tests first:* gate rule matrix (every operator, pass and fail); `survey` golden on each fixture; `next_actions[0]` is the highest-severity new finding in a constructed envelope.

#### P2.10 First dogfood

**Status:** not started

- `cupel init --write` on the Cupel repo (commit the resulting `cupel.toml`); `[gate.changed]` with `tests.weakening = "none"` and `static.crap.max < 30` (CC-only until P3).
- `make dogfood` target; CI job runs it; `cupel check --depth quick` on the whole tree is clean or every finding is either fixed or suppressed with a reason.

#### P2 Definition of Done

- All seven static checks pass goldens in all five languages (or have a tested `skipped` reason via `Capabilities`).
- `cupel survey --depth quick` on the Cupel repo completes in < 5 s.
- `cupel check --changed --gate` passes in CI on the Cupel repo.
- `describe` covers every shipped check and finding kind.
- README: full check table, `survey`, `explain`, `baseline`, `suppress`, gate configuration, agent loop.

#### P2 Notes

_(none yet)_

---

### P3 — Coverage and mutation

**Goal:** Tier 2–3 evidence: real test execution, coverage ingestion, per-test maps, impacted-test selection, coverage-gated differential mutation testing, flakiness detection, and the mandatory exit-4 refusal when the baseline suite is red.

**Status:** not started

External tools used here (`cargo-llvm-cov`, `cargo-nextest`, `go`, `pytest`, `pytest-cov`, `vitest`, `jest`) are exercised for real in integration tests when present and reported as skipped when absent. Parsers are always tested against captured real output committed as fixtures.

#### P3.1 Runner adapters and baseline test run (exit 4)

**Status:** not started

- `RunnerAdapter` trait (design §8.6); adapters for `cargo test`, `cargo nextest`, `go test -json`, `pytest`, `vitest`, `jest`, registered in the P1.9 toolchain table; `list()` and `run(selection, env, timeout)` → `TestResults` (per-test outcome, duration).
- Baseline test run executed once in the real tree before any check that needs it; red baseline → `status: error`, exit 4, no scores produced. `test_results` table.
- `doctor` gains tool presence/version checks for configured runners.

*Tests first:* parser fixtures for each runner's output format; a fixture with one failing test → `check --only tests.mutation` exits 4 and emits zero mutation findings.

#### P3.2 Coverage adapters, `tests.coverage`, full `static.crap`

**Status:** not started

- `CoverageAdapter` trait; parsers for LCOV, Cobertura XML, Istanbul JSON, go coverprofile, JaCoCo XML → `CoverageMap` (line + branch). Producers for Rust (`cargo llvm-cov --lcov`), Go (`go test -coverprofile`), Python (`pytest --cov --cov-report=lcov`), JS/TS (`vitest --coverage`, `jest --coverage`, `c8`). Ingest existing reports when fresher than sources (mtime + content hash). `Capabilities.branch_coverage` is false for Go (coverprofile is statement-level) and `coverage.uncovered-branch` is skipped there with that reason.
- `tests.coverage` check with `coverage.uncovered-symbol`, `coverage.uncovered-branch`; `coverage` table.
- `static.crap` consumes the coverage adapter; `--no-coverage` retained.

*Tests first:* each parser against committed real reports; `CoverageMap` property (line hits are non-negative and symbol coverage ∈ [0,1]); CRAP golden with known coverage.

#### P3.3 Per-test coverage maps and `tests --impacted`

**Status:** not started

- Per-test maps: nextest + llvm-cov per-test profiles (Rust), `go test -coverprofile -run '^TestX$'` per test (Go, `PerTestCoverage::ByNarrowing`), pytest-cov `--cov-context=test` (Python); JS/TS file-level fallback over the P1.7 resolved import graph with `per_test_map: false` echoed. Write **ADR-006**.
- Inverted index `line → tests`; `tests --impacted` returns `{id, file, reason}` plus a ready-to-run `command` per runner; says when it fell back to file-level.

*Tests first:* a fixture with two tests covering disjoint functions — changing one function yields exactly one impacted test.

#### P3.4 `tests.mutation`

**Status:** not started

Built in sub-steps, each with its own failing test first:

1. **Sites → mutants**: from P1.5 sites, generate mutant source for each; `mutants` table.
2. **File-copy overlay**: reflink (APFS/btrfs/XFS) else hardlink tree + private copy of mutated file plus everything `overlay_deps()` (P1.7) names. Mutants of one file run concurrently, files sequentially.
3. **Coverage gating**: uncovered sites emit `mutant.uncovered` and are never executed.
4. **Per-test selection**: only tests covering the mutated line run; full suite when no per-test map.
5. **Timeouts**: `timeout_factor × baseline_duration(test)`, floor 2 s → `mutant.timeout`.
6. **Store-based differential reruns**: outcome reused when `(symbol_hash, site_hash, suite_hash, toolchain_hash)` unchanged; scores recomputed from the store.
7. **Ordering**: last run's survivors → sites in changed symbols → rest.
8. **Findings**: `mutant.survived` (with `from`, `to`, `operator`, `covering_tests`, `nearest_test_file` hint), `mutant.timeout`, `mutant.uncovered`, `mutant.equivalent?` per **ADR-004**. Score = killed / (killed + survived).
9. Depth mapping: `standard` = changed symbols, `deep` = touched files, `exhaustive` = tree.

*Tests first:* a fixture per language with a function and a deliberately weak test suite where the surviving set is known exactly; golden asserts the exact survivors; a second run with no changes executes zero mutants (differential); a change to one test re-executes only the sites it covers.

#### P3.5 `tests.flaky`

**Status:** not started

- Rerun impacted tests N times (default 3 at `standard`) with shuffled order, varied seeds, varied parallelism. Kinds `test.flaky`, `test.order-dependent`; action `investigate_flake`.

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

_(none yet)_

---

### P4 — Schemata, daemon, budget scheduler

**Goal:** Compile-once mutation for Rust, a long-lived daemon with byte-identical output, and value-ordered scheduling under a wall-clock budget.

**Status:** not started

#### P4.1 Rust mutant schemata

**Status:** not started

- `schemata()` on the Rust adapter: rewrite each site as `if cupel_mutant_id() == N { mutated } else { original }` preserving expression types; compile once; run each mutant with `CUPEL_MUTANT=N`. Validate the schemata build compiles before running; fall back to file-copy on failure (recorded in the check result). Write **ADR-005** on Go timing.

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

- `cupel-plugin`: `Manifest`, `RunRequest`, event types; discovery via `[plugins]` and `cupel-check-*` on `$PATH` (`--no-path-plugins`); `describe`/`plan`/`run` subprocess protocol over stdio per §8.5; budget enforcement (SIGTERM then SIGKILL); manifest schema validation; undeclared finding kinds rejected; non-zero exit without `result` → `status: error`.
- Reference plugins in Rust, Python, and TypeScript under `crates/cupel-fixtures/plugins/`, each a trivial check, used by a protocol conformance suite. Property tests on request/event round-trips.

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

- Conformance suite green for all three reference plugins.
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

#### P6.3 `conc.race`, `perf.bench`, `gen.fuzz`

**Status:** not started

- Bench adapters (criterion, benchstat, hyperfine) with statistical comparison against baseline.

#### P6.4 Java and Clojure adapters

**Status:** not started

- Each added by following the design §8.3.4 checklist verbatim, as the proof that the extension point works: new `langs/<id>/` directory, fixture repo with full `inventory.toml`, toolchain table entry (JUnit/Maven/Gradle + JaCoCo; `clojure.test` + Cloverage), conformance green, all goldens. Java mutation delegates to PIT via a wrapping plugin. Go schemata per ADR-005 if deferred from P4.
- Record in Notes the exact set of files touched outside `langs/`, `cupel-fixtures/`, and the toolchain table. The target is zero; anything else is an extension-point defect to fix.

#### P6 Definition of Done

- Every check in §5.1, §5.3 (except `gen.symbolic`), §5.4 `conc.race`, §5.6 has goldens; seven languages pass the P2 suite and the conformance suite.
- README: full language and check matrix.

#### P6 Notes

_(none yet)_

---

### P7 — Formal, distributed, MCP

**Status:** not started

#### P7.1 `spec.contracts` · P7.2 `spec.trace` (ADR-007) · P7.3 `conc.linearizability` · P7.4 `conc.dst` · P7.5 `conc.model-check` and `spec.prove` · P7.6 `meta.traceability` (ADR-008) and `meta.oracle-independence` · P7.7 `serve --mcp`

**Status:** not started (each sub-step gets its own status line when P7 begins)

Each wrapped tool follows the adapter rule: parser tested on committed real output; integration test runs the real tool when installed, otherwise reports skipped visibly.

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
- README verb/check/exit-code tables generated from `describe --all` by a `make readme` target; a test fails if the README is stale.

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
- [ ] `describe` reflects any new verb, flag, check, or finding kind
- [ ] Goldens updated deliberately (`UPDATE_GOLDENS=1`) and the diff reviewed, never blindly
- [ ] Status line and Notes in this document updated
- [ ] ADR written if a decision was made
- [ ] README reflects what exists (at phase boundaries at minimum)
- [ ] Committed
