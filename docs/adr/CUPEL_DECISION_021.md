# CUPEL_DECISION_021: Project detection and the `cupel.toml` that `init` writes

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.8

## Context

Design §6.5 says `init` "detects project roots (Cargo workspace, go.work, npm/pnpm/yarn workspaces, uv/poetry/pip, Maven/Gradle, deps.edn), languages, test command, coverage command, benchmark command", writes `cupel.toml` and `.cupel/.gitignore` only with `--write`, shows the file under `--dry-run`, and never overwrites `cupel.toml` without `--force`. Design §8.3.2 makes this knowledge the `ProjectInfo` fact, produced per language, and §8.7 puts the runner, coverage, and bench choices in a toolchain table in `cupel-adapt`. Neither crate exists in P0, and P1.9 moves the detection there. Writing the code requires answers the design does not give:

1. Which crate holds detection in P0, and what seam P1.9 moves.
2. What makes a directory a root, and when a nested manifest is a separate root versus a member of an enclosing workspace.
3. The exact default commands per language and the rule that picks each one.
4. How the detected roots become a `cupel.toml`: `[roots]` names, what happens when two roots share a language (the file has one `[lang.<id>]` table per language), and which keys `init` writes at all.
5. What `cupel init` does with neither `--write` nor `--dry-run`.
6. What `init` does when an existing configuration does not load, and how it learns `default_branch`.

## Options

For (1): `cupel-core` is ruled out by ADR-011 (core names no language). A new `cupel-lang` crate now would front-run P1.1's design of the `Language` trait. `cupel-cli`, the one crate allowed to depend on everything, can hold a standalone module whose output is already the core `ProjectInfo` payload, so P1.9 moves code without changing what `init` consumes.

For (2): treat every manifest as a root (a Cargo workspace with ten members would yield eleven roots), parse each ecosystem's workspace declaration (`[workspace].members` globs, `go.work` `use`, `package.json` `workspaces`, `pnpm-workspace.yaml`), or use nesting: a manifest below an existing root of the same ecosystem belongs to that root. Parsing every workspace format is four glob dialects for little gain at `init` time; nesting gives the same answer for every real workspace layout, and a stray nested manifest at worst becomes a member rather than a root, which the user can correct in the generated file.

For (3): one fixed command per language (simple, often wrong: `cargo nextest` fails where nextest is not set up), or detection by the lockfile, configuration file, and dependency names that §8.7 calls the table's "detection rules".

For (5): a usage error naming `--write` and `--dry-run` (design §6.1 rule 2: a needed decision exits 2 naming the resolving flag), or a preview that behaves like `--dry-run` and says how to apply it. Nothing is ambiguous about what the user wants to see, and a preview carries the diff an agent needs to decide; an error would only send it round again with `--dry-run`.

## Decision

1. **Location and seam.** Detection lives in `crates/cupel-cli/src/detect.rs` with one submodule per ecosystem (`cargo`, `go`, `node`, `python`, `jvm`). `detect::project(root) -> cupel_core::syntax::ProjectInfo` is the whole API; the payload type is used unchanged. Each submodule exports an `Ecosystem { manifests, skip_dirs, describe }`, where `describe` turns a candidate root (its directory and the member directories found under it) into a `ProjectRoot`. In P1.9 each `describe` becomes the body of that language's `ProjectInfo` producer in `cupel-lang`, and the command choices inside it move into the `cupel-adapt` toolchain table; `init` then reads the `ProjectInfo` fact instead of calling `detect::project`.
2. **Roots.** Detection walks the repository from the root, skipping hidden directories and every ecosystem's skip directories (`target`, `node_modules`, `vendor`, `venv`, …), in sorted order. A directory containing one of an ecosystem's manifests is a root of that ecosystem unless one of its ancestors already is, in which case it is a member of that root. Manifests: Cargo `Cargo.toml`; Go `go.mod`, `go.work`; Node `package.json`; Python `pyproject.toml`, `setup.cfg`, `requirements.txt`; JVM `pom.xml`, `build.gradle`, `build.gradle.kts`, `settings.gradle`, `settings.gradle.kts`; Clojure `deps.edn`. Roots are ordered by path, then language. A root's `build_files` are its manifests, then its members' manifests, relative to the root.
3. **Languages and commands.** Commands run from the root's directory. `{out}`, `{name}`, and `{id}` are placeholders the producers fill (design §6.7).

   | Language | Rule | test | coverage | bench |
   |---|---|---|---|---|
   | `rust` | `.config/nextest.toml` at the root | `cargo nextest run --workspace`, else `cargo test --workspace` | `cargo llvm-cov --lcov --output-path {out}` | `cargo bench --bench {name} -- --save-baseline {id}` when any manifest names `criterion`, else none |
   | `go` | `go.work` `use` directories, else `./...` | `go test -json <pkgs>` | `go test -coverprofile={out} <pkgs>` | `go test -run=^$ -bench=. <pkgs>` |
   | `typescript` if a `tsconfig.json` sits beside the root's or a member's `package.json`, else `javascript` | runner by dependency name (`dependencies` or `devDependencies` of any manifest): vitest, else jest, else mocha, else `node:test`; package manager pnpm (`pnpm-lock.yaml`, `pnpm-workspace.yaml`, or `packageManager: pnpm@…`), yarn (`yarn.lock` or `packageManager: yarn@…`), else npm, which executes tools with `npx` | `<x> vitest run`, `<x> jest`, `<x> mocha`, `node --test` | `<x> vitest run --coverage --coverage.reporter=lcov`, `<x> jest --coverage --coverageReporters=lcov`, `<x> c8 --reporter=lcov mocha`, `<x> c8 --reporter=lcov node --test` | `<pm> run bench` when the root's `package.json` has a `bench` script, else none |
   | `python` | uv (`uv.lock` or `[tool.uv`), poetry (`poetry.lock` or `[tool.poetry`), else pip; pytest when `pytest` appears as a name in `pyproject.toml`, `setup.cfg`, or `requirements*.txt`, or `pytest.ini`/`conftest.py` exists, else unittest | `<m> pytest`, else `<m> unittest` | `<m> pytest --cov --cov-report=lcov:{out}`, else `<m> coverage run -m unittest && <m> coverage lcov -o {out}` | `<m> pytest --benchmark-only --benchmark-json={out}` when `pytest-benchmark` appears, else none |
   | `java` (Maven, Gradle), `clojure` (`deps.edn`) | recognized as roots only | none | none | none |

   `<m> mod` is `python -m mod`, `uv run python -m mod`, or `poetry run python -m mod`. A Python dependency name "appears" when it occurs bounded by characters that cannot be part of a package name (so `pytest-benchmark` does not count as `pytest`); Python requirement strings would need a PEP 508 parser to do better, and a false positive only costs a command the user edits. `package.json` is parsed as JSON.
4. **The generated file.** In this order: a one-line comment pointing at `cupel describe schema config`; `default_branch` when git knows it; `depth = "standard"`; `[roots]` unless there is exactly one root and its path is `.`; then one `[lang.<id>]` table per language with commands, keys `test`, `coverage`, `bench`, omitting absent commands. A root's name in `[roots]` is its directory's last component (the language ID for `.`), suffixed `-<lang>` on collision and quoted when it is not a bare TOML key. When several roots share a language, that language's table takes the commands of the first such root in path order; the file has one table per language (design §6.7), so per-root commands wait for a design change. `required_version` is not written (it would pin every golden to the release that generated it), nor are skip directories (no configuration key holds them; the P1 walker reads them from `ProjectInfo`). A test loads every generated file with `Config::load`.
5. **Writing.** `cupel.toml` at the root and `.cupel/.gitignore`, both through the run's `Writer`. With `--write` the files are written and the run keeps its run record as usual. Without `--write`, whether or not `--dry-run` is given, `init` is a **preview**: the writer records instead of writing, the envelope is the `plan` envelope of a dry run with `would_write` diffs, exit 0, and it carries one `next_actions` entry (`kind: configure`, command `cupel init --write`). A preview, like a dry run, writes no run record, so the tree stays byte-identical.
6. **Existing configuration.** When `cupel.toml` exists at the root and `--force` is absent, `init` exits 2 with a `cli.usage` finding whose subject and argument are `--force`, in previews too. `init` runs even when configuration does not load (it is the verb that replaces a broken file); configuration errors are then ignored and `run.config` stays empty.
7. **`default_branch`** comes from git, run in the root: the branch `refs/remotes/origin/HEAD` points at; else `main` or `master` if that local branch exists; else the branch `HEAD` names. Outside a repository, or without `git`, the key is omitted. Cupel shells out to `git` here as an interim measure; ADR-002 (P2.1) decides git access for good and replaces this call.

## Consequences

- P1.9 moves five small modules and keeps `init`'s goldens unchanged, which its step text already requires.
- Generated configurations name `java` and `clojure` roots before those languages exist. When P1.1 makes the CLI call `Config::validate_registered`, those IDs must be registered (perhaps without producers) or `init` must leave such roots out until P6; P1.1 decides.
- Goldens run `init` in a temporary copy of each fixture, outside any git repository, so `default_branch` is absent from them; a separate test covers the git lookup.
- Agents can run `cupel init` safely: it never writes without `--write`.
