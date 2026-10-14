# Cupel

Cupel is a verification suite that grades what a coding agent produced. Functional tests tell you whether code works; Cupel adds the other ways of knowing — structural metrics, test-suite strength, mutation testing, generative testing, concurrency checks, formal specs, performance regression — behind one CLI with one output contract. An agent can learn the tool from the tool, run anything without side effects first, and always get back a ranked list of what to do next.

A *cupel* is the porous cup used in fire assay to separate precious metal from base.

```sh
cupel init --write      # detect your project, write cupel.toml
cupel doctor            # verify the environment
cupel describe          # learn the tool from the tool
```

## Status

The platform and the language layer are built: the binary, the output contract every verb returns, self-description, configuration, project detection, the run infrastructure (dry runs, event streaming, timeouts, the `.cupel/` directory), and language support for JavaScript, TypeScript, Rust, Go, and Python that produces every syntactic fact the checks will consume. **No checks exist yet.** `check`, `survey`, `explain`, `baseline`, `oracle`, `tests`, `suppress`, and `serve` parse and describe themselves but exit 2 with a `cli.not-implemented` finding that names the plan phase that builds them.

- [Technical design](docs/CUPEL_TECHNICAL_DESIGN.md) — the specification: CLI, output contract, check catalog, architecture.
- [Implementation plan](docs/CUPEL_IMPLEMENTATION_PLAN.md) — the phased, test-driven build plan, kept current as work proceeds.
- `docs/adr/` — architecture decision records written as implementation raises questions the design does not answer.

This README describes only functionality that exists.

## Install

From source, with a Rust toolchain (the pinned version is in `rust-toolchain.toml`):

```sh
git clone https://github.com/tysont/cupel
cd cupel
make install            # cargo install --path crates/cupel-cli
```

## Usage

### `cupel init`

Detects project roots and languages — Cargo (single crate or workspace), Go modules and workspaces, npm/pnpm/yarn packages and workspaces (TypeScript when `tsconfig.json` is present), Python (`pyproject.toml`, `setup.cfg`, `requirements.txt`), and Maven, Gradle, and `deps.edn` roots — and chooses default test, coverage, and bench commands per language.

```sh
cupel init              # preview: the envelope carries unified diffs of what would be written
cupel init --dry-run    # same preview, explicitly
cupel init --write      # write cupel.toml and .cupel/.gitignore
cupel init --write --force   # replace an existing cupel.toml
```

A generated `cupel.toml` for a monorepo looks like this:

```toml
default_branch = "main"
depth = "standard"

[roots]
engine = { path = "engine", lang = "rust" }
svc = { path = "svc", lang = "go" }
web = { path = "web", lang = "typescript" }

[lang.go]
test = "go test -json ./..."
coverage = "go test -coverprofile={out} ./..."

[lang.rust]
test = "cargo test --workspace"
coverage = "cargo llvm-cov --lcov --output-path {out}"

[lang.typescript]
test = "npx vitest run"
coverage = "npx vitest run --coverage --coverage.reporter=lcov"
```

### `cupel doctor`

Verifies the binary against `required_version`, the validity of `cupel.toml`, write access to `.cupel/`, and the presence of required tools. Every problem is a finding with an `install_tool` or `configure` action; exit 3 means something required is missing.

### `cupel describe`

Every verb, flag, exit code, and schema is described by the binary itself, and the human help is rendered from the same data so the two cannot drift.

```sh
cupel describe                     # the CLI: verbs, global flags, exit codes, schemas
cupel describe init                # one verb: flags with types, defaults, env vars, examples
cupel describe schema run          # a JSON Schema (draft 2020-12): run | finding | events | config | plugin
cupel describe --all               # everything, one JSON document
cupel init --describe              # any verb describes itself instead of running
```

### Global flags

| Flag | Meaning |
|---|---|
| `--format json\|jsonl\|text` | JSON when stdout is not a terminal, text when it is; `CUPEL_FORMAT` also sets it |
| `--dry-run` | Preview what would run or be written; change nothing |
| `--config PATH`, `--profile NAME` | Use this `cupel.toml` or apply `[profile.NAME]` |
| `--cwd PATH` | The repository root; Cupel never relies on the shell's working directory |
| `--events PATH\|stderr` | Stream progress as JSON lines; stdout stays reserved for the envelope |
| `--timeout DURATION` | Stop after this long with partial results and exit 5 |
| `--seed N` | Seed for generative checks; random by default, always echoed |
| `--limit N`, `--offset N` | Page findings; the envelope always carries `truncated` and `total_findings` |
| `--write` | Permit writes outside `.cupel/`; nothing else ever writes elsewhere |
| `--quiet`, `--verbose`, `--no-color` | Diagnostics on stderr only; stdout's shape never changes |

## Languages

Cupel understands **JavaScript, TypeScript, Rust, Go, and Python** out of the gate. Each language is a tree-sitter grammar plus a declarative spec (queries and tables), and produces the same language-neutral facts: `symbols`, `decisions`, `norm-tree` (the structural hash that gives every function an identity surviving rename, move, and reformat), `mutation-sites`, `test-frames`, `test-conventions`, `module-graph` (imports resolved to files, with Node, `tsconfig` `paths`, Go module, Rust `mod`, and Python package resolution), `api-surface`, `roots`, and `project-info`. Checks consume facts and never see a language, so every check works for every language that produces what it needs.

```sh
cupel describe languages           # the five languages, one line each
cupel describe language go         # extensions, facts produced, toolchain row (runners, coverage, bench, fuzz, race)
cupel describe fact norm-tree      # a fact's schema, version, producers per language, consumers
cupel describe matrix              # checks × languages — empty until the first check lands
```

Every language is held to the same conformance inventory: a shared list of constructs per fact that each language must exhibit in its fixture repository and detect, or mark not-applicable with a reason. Adding a language is a new `langs/<id>/` directory, a fixture repository, and a toolchain-table row — nothing in the core, the checks, or the CLI changes.

## Output contract

Every invocation — success, usage error, unbuilt feature, even an internal panic — writes one JSON envelope to stdout. Agents parse one shape and branch on `status` and `exit_code`.

```json
{
  "cupel": { "version": "0.1.0", "schema": "https://cupel.dev/schema/v1/run.json" },
  "run": {
    "id": "run_01M4KN0Q2TZKN3G8NBGPB15C0D", "verb": "init",
    "started": "2026-10-10T19:35:40Z", "duration_ms": 59,
    "depth": "standard", "seed": 15619067772698868725,
    "config": { "effective": { "…": "…" }, "sources": ["flags", "cupel.toml", "defaults"] }
  },
  "status": "plan",
  "exit_code": 0,
  "checks": [],
  "findings": [],
  "plan": {
    "would_write": [
      { "path": "cupel.toml", "diff": "--- /dev/null\n+++ b/cupel.toml\n…" },
      { "path": ".cupel/.gitignore", "diff": "…" }
    ],
    "would_run": [], "would_skip": [], "would_install": [], "est_total_ms": 0
  },
  "next_actions": [
    { "priority": 1, "kind": "configure", "command": "cupel init --write",
      "summary": "Nothing was written; run `cupel init --write` to write these files." }
  ],
  "truncated": false,
  "total_findings": 0
}
```

Every object has a fixed key set; absent values are `null`, never missing. `status` is one of `pass | fail | partial | error | plan`. Each finding carries a stable ID, severity, confidence, evidence tier, location, an action with a closed `kind`, and a `repro` command. `--format text` renders the same envelope for people; `--format jsonl` emits the envelope header then one finding per line.

### Exit codes

| Code | Name | Meaning |
|---|---|---|
| 0 | pass | Ran fully; gate passed or no gate |
| 1 | fail | Ran fully; gate failed |
| 2 | usage | Bad flags or config; the envelope names the fix |
| 3 | environment | Missing tool, oracle, or toolchain; `doctor` findings attached |
| 4 | unevaluable | Baseline test suite fails or build broken; scoring refused |
| 5 | partial | Timeout or budget hit; the envelope has valid partial results |
| 6 | internal | Cupel bug; the envelope includes a bug-report payload |

## Configuration

`cupel.toml` is discovered by walking up from `--cwd`. Precedence is flags > `CUPEL_*` environment variables (`CUPEL_DEPTH`, `CUPEL_FORMAT`, `CUPEL_BUDGET`, `CUPEL_PROFILE`) > `--profile` > `cupel.toml` > defaults, and the envelope echoes the effective configuration and which layers contributed under `run.config`. Unknown keys are usage errors that list the valid keys. `cupel describe schema config` is the full schema.

Cupel writes only under `.cupel/` (run records, caches, scratch space) unless `--write` is given. `.cupel/.gitignore` excludes everything except `suppressions.toml`.

## Principles

- **One binary, zero runtime dependencies.** Drops into any repository an agent is working in.
- **One output envelope.** Every verb returns the same JSON shape; every finding carries a stable ID, a severity, an evidence tier, a concrete action, and a reproduction command.
- **Self-describing.** Every verb, flag, check, finding kind, exit code, and schema is documented by the binary itself.
- **Side-effect free by default.** `--dry-run` on every verb; nothing is written outside `.cupel/` without `--write`.
- **Cupel grades; the agent fixes.** No fix mode, no interactive mode, no dashboard.

## Development

```sh
make check      # fmt, clippy (-D warnings), tests, docs, and a clean-tree check — the pre-commit bar
make test       # cargo test --workspace; UPDATE_GOLDENS=1 make test rewrites golden envelopes
make lint       # cargo fmt --check and clippy
make build      # cargo build --workspace
```

The workspace is a set of crates under `crates/`: `cupel-core` (the output contract, fact vocabulary, and extension model), `cupel-lang` (the `Language` trait, generic query-driven producers, and one `langs/<id>/` directory per language), `cupel-adapt` (the toolchain table of runners, coverage, and bench tools per language), `cupel-cli` (the binary), and `cupel-fixtures` (fixture repositories, conformance inventories, and golden envelopes). `cupel-lang` and `cupel-adapt` depend on `cupel-core` alone and never on each other. Every behavior is built test-first; see the implementation plan for the phase structure and the definition of done at each step.

## License

BSD 3-Clause. See [LICENSE](LICENSE).
