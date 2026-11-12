# CUPEL_DECISION_167: Configured `[roots]` are the project's roots, and `init` makes each member of a workspace that delegates its tests a root

**Date:** 2026-11-10 · **Status:** accepted; amended by [177](CUPEL_DECISION_177.md) and [179](CUPEL_DECISION_179.md) (owner rulings, 2026-11-11) · **Phase:** v0.2.6, Molten field trial Area 5 · Amends [021](CUPEL_DECISION_021.md) §3–4 (nesting decides members; commands from a language's first root) and [037](CUPEL_DECISION_037.md) (`ProjectInfo` reports detected facts only)

## Context

`cupel.toml`'s `[roots]` table (`name = { path, lang }`) was validated and echoed under `run.config`, but nothing read it. Every runner, coverage tool, and walk took its roots from detection, where a directory nested under another's manifest is a member of it, not a root.

On the owner's Molten repository (a pnpm workspace whose root `package.json` runs `pnpm -r test`):
- ADR-162 rightly defers the root's Vitest, which would sweep every package and the deployed-environment API tier from the root;
- ADR-166 makes that deferral hold at run time.

That left Molten with no runner at all. Each package runs its own Vitest under its own configuration, and Cupel had no way to say so: configured roots were ignored, and `init` wrote one root at `.`.

## Decision

1. **Configured roots are the roots.** When `[roots]` names any root, each language's `ProjectInfo` holds exactly the configured roots of that language, and nothing is detected. Each is described as a detected root at that directory would be: its build files, and the manifests below it as members.
   - A configured path without one of the ecosystem's manifests is no root.
   - So is one the ecosystem describes in another language: an ECMA directory is TypeScript when it holds a `tsconfig.json`.

   `Ctx` carries the configured roots, so every producer and verb that reads `ProjectInfo` sees them:
   - the scheduler;
   - `doctor`;
   - `tests --impacted`;
   - the daemon's warm-up;
   - scope resolution.
2. **`init` splits a workspace that delegates its tests.** A detected root whose runner is deferred (its `test` script names another command) and that has members is replaced by its members, each a root in the language its own manifests give. `init` then writes them under `[roots]`, and each language's commands from its first root, as before.

   With no members to split into, the root keeps no command, and `init` reports `init.runner-deferred` (ADR-162).
3. **A runner is judged where it would run.** With configured roots, a runner's detection holds when it holds in any configured root of its languages, and its deferral (ADR-166) when it holds in every one. A per-test map producer follows its runner. Without configured roots, both are judged at the repository root, as before. So a root `pnpm -r test` defers Vitest only where the repository root is where it would run.
4. **Narrowing a run** to some packages is editing `[roots]`. A `crates/cupel-cli/tests/init.rs` test with a recording Vitest in each of two packages runs only the configured one's.

## Consequences

- Molten's `init` writes eight roots:
  - TypeScript: `apps/agent`, `apps/cli`, `apps/web`, `packages/shared`, `poc`, `tools/discovery`, with `npx vitest run`;
  - JavaScript: `containers/sandbox` and `poc/containers/echo-test`, with `node --test`.

  `tests/api` and `tests/e2e`, which reach deployed environments, are in no root.
- A language still has one runner and one command for all its roots (ADR-021). A root of that language that has no tests or no such runner (Molten's `tools/discovery`) still gets the command. Per-root commands remain a later step.
- The launcher is detected in each member's own directory. A pnpm member without its own lockfile gets `npx`, which runs the member's locally installed binary.
- A `[roots]` that names no root of a language leaves that language without roots, as configured.
