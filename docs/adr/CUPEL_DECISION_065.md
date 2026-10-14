# CUPEL_DECISION_065: `[scope] exclude` and the directories every run excludes

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.10

## Context

The first dogfood run over the Cupel repository analysed the fixture corpora under `crates/cupel-fixtures/` (five languages, 356 files) as if they were Cupel's code, and `scope.roots` listed every fixture repository as a project root. Nothing could express "never look here": `[roots]` names roots but does not limit the walk (and the Cargo workspace root `.` contains the fixtures), `ProjectInfo.skip_dirs` holds only directory *names* an ecosystem generates below its own roots, and `--paths` selects but cannot negate and must be repeated on every invocation. Design §6.7 has no exclusion key. The project owner ruled that the mechanism is a `[scope]` table with `exclude` globs, that root detection must not descend into excluded paths, and that vendored dependencies, build output, and `.cupel` are always excluded.

## Options

1. A top-level `exclude` key. Works, but the top level of `cupel.toml` holds layered settings (ADR-017); scoping is a separate concern that may grow (an `include` later).
2. A `[scope]` table with `exclude`, gitignore-style globs compiled with `globset`, applied wherever a run reads the repository: the file walk, project-root detection, and the changed paths of a range.
3. Reuse `.gitignore`. Fixtures are committed, so they cannot be ignored by git; a separate `.cupelignore` file adds a second configuration surface.

## Decision

Option 2.

- **Syntax.** `[scope] exclude = ["…"]`; `[scope]` rejects other keys. A pattern containing `/` (other than a trailing one) is anchored at the repository root (a leading `/` is allowed); a pattern without one matches a name at any depth. `*` does not cross `/`, `**` does. A path is excluded when it or any directory above it matches, so `crates/cupel-fixtures` and `crates/cupel-fixtures/**` both exclude everything inside that directory (as in gitignore, `dir/**` does not match `dir` itself, which does not matter because nothing inside it is kept). An invalid glob is a configuration error (exit 2) naming the pattern.
- **Always excluded** (`cupel_core::ALWAYS_EXCLUDED`), at any depth: `.cupel`, `node_modules`, `target`, `vendor`. Each ecosystem's other generated directories (`dist`, `coverage`, `.venv`, `__pycache__`, …) are skipped as before: anywhere by root detection, below that ecosystem's roots by the file walk (ADR-037).
- **Where it applies.** `cupel_core::Exclude` travels in the producers' `Ctx` (new field `exclude`; `Ctx::new(root)` excludes only the built-ins). `cupel_lang::walk_excluding` prunes excluded directories and drops excluded files; `cupel_lang::project::roots` and `unregistered_roots` never enter an excluded directory, so `scope.roots`, `doctor`, and the languages in scope ignore excluded projects; `check` and `survey` drop excluded paths from a range's changes, so editing an excluded file is not a change in scope (and is not checked out as a base file). `init` runs before a configuration exists and uses the built-ins only.

## Consequences

- Cupel's own `cupel.toml` excludes `crates/cupel-fixtures/**`, which also removes the fixture crate's harness code from the dogfood; the corpora are analysis data, not Cupel's code (P1 Notes).
- `--paths` that match only excluded files are a usage error ("matches no source file"), because excluded files are not in the walk.
- A file inside an excluded directory can no longer be analysed by any flag; to analyse it, change the configuration.
