# CUPEL_DECISION_168: `perf.footprint` reads the `pnpm-lock.yaml` pnpm installs a root from

**Date:** 2026-11-10 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 5 · Amends [103](CUPEL_DECISION_103.md) (the JavaScript and TypeScript ecosystem)

## Context

The npm ecosystem of `tool.footprint` read a root's `package-lock.json`, else its `package.json`'s declared names without versions. A pnpm project keeps neither lockfile: `pnpm-lock.yaml` holds the resolved graph, and a workspace keeps one at its root for every member.

On the owner's Molten repository (a pnpm workspace, eight roots since ADR-167):
- `perf.footprint` counted 93 dependencies;
- the pnpm members contributed only the names they declare, so there were no versions for advisories to match and no licenses;
- the two npm roots with a `package-lock.json` (`tools/discovery`, `containers/sandbox`) contributed their full graphs, so the count mixed two units;
- each root's dependencies were appended to the others', so a package two members share would have been counted twice;
- a member's `workspace:*` link to another member was counted as a dependency.

## Decision

1. **The lockfile pnpm installs from.** A JavaScript or TypeScript root without a `package-lock.json` uses the `pnpm-lock.yaml` of the nearest directory at or above it that holds a `pnpm-workspace.yaml`, else its own. The rule applies only when that lockfile has an importer for the root. Otherwise the root falls back to its `package.json`, as before.

   A lockfile left inside a workspace member is not what pnpm installs it from, and is not read. Neither is the workspace's lockfile for a directory it does not list as a member.
2. **What an importer locks.** The root's importer contributes:
   - its `dependencies`, `devDependencies`, and `optionalDependencies`, direct, each at its resolved version;
   - then, transitively, what each package's snapshot needs (`dependencies` and `optionalDependencies`), each once, at its line under `packages`.

   Versions drop their peer suffix (`9.39.2(jiti@2.6.1)` is `9.39.2`). `link:` versions are the workspace's own packages and are left out.

   The reader follows the block layout pnpm writes for lockfile v9: top-level sections, two spaces per level. It is not a YAML parser, and no dependency is added.
3. **Licenses from the install.** `pnpm-lock.yaml` records no licenses. When `node_modules/.pnpm` exists beside the lockfile, each installed package's `package.json` gives its license, by `name@version`.
4. **One entry per package of a file.** The producer merges what its roots list, both at head and at the base: a package the same file lists again at the same version is kept once, and it is direct when any root declares it. Manifests are listed once.

## Consequences

- On Molten, `perf.footprint` counts 487 dependencies, all with versions. A separate Ruby computation over the same lockfiles with a real YAML parser gives the same 487:
  - 441 locked in the workspace for its five pnpm members;
  - 40 from `tools/discovery/package-lock.json`;
  - 5 from `containers/sandbox/package-lock.json`;
  - 1 declared by `poc/containers/echo-test`, which is not a workspace member.
- Advisories configured under `[lang.<id>] advisories` can now match pnpm projects' dependencies.
- The workspace root's own importer (`.`) is read only when `.` is a configured root.
- Lockfile versions before 9 have the same `importers` but name packages under `packages` with a leading `/`. Their importers are read, but their transitive closure is not. They can be added when a project needs them.
