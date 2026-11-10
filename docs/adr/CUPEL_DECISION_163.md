# CUPEL_DECISION_163: The files a project's manifests say it runs root their modules, and dead code spans the ECMA family

**Date:** 2026-11-09 · **Status:** accepted · **Phase:** v0.2.6, Molten field trial Area 2 · Extends [058](CUPEL_DECISION_058.md) (what roots a module) and [037](CUPEL_DECISION_037.md) (`ProjectInfo`); applies [051](CUPEL_DECISION_051.md)'s family span to `static.dead-code`

## Context

On the owner's Molten repository, about 80 of `static.dead-code`'s 91 `dead.symbol` findings were false. Two causes:

1. **A file the project runs directly was an unimported module.**
   - JavaScript and TypeScript roots a module only through a function named `main`, a default export the API query marks as a hook, an export, or a test.
   - Nothing said that a file is what the project runs, so these were reported dead module-wide:
     - the 59 functions of `containers/sandbox/server.js`, which a Dockerfile copies into its image and an entry script runs with `node`;
     - the code of each Cloudflare Worker's entry file (`wrangler.toml`'s `main`).
2. **A family member's facts were invisible.**
   - Each language's dead-code task read only its own language's facts.
   - Molten's root has a `tsconfig.json`, so its one ECMA root is a TypeScript root. Its JavaScript files (the sandbox's) saw no `ProjectInfo` root and no TypeScript import edges.
   - So a JavaScript module imported only from TypeScript, or named as an entry only by the TypeScript root's manifests, looked unreachable.

## Options

**What says a file is run.**
- (a) Name-based conventions (`server.js`, `index.ts`). A guess.
- (b) The project's own manifests: what the project declares it runs or ships.

Chosen: (b).

**Across languages.**
- (a) Copy the TypeScript root into JavaScript's `ProjectInfo`. Runners read `ProjectInfo` too, and a JavaScript runner would then run at the TypeScript root.
- (b) Make `static.dead-code` span the family, as `static.clones` does (ADR-051). It then reads every family member's symbols, trees, imports, roots, references, test conventions, and project roots as one.

Chosen: (b). Reachability is a question about the whole program, and JavaScript and TypeScript are one program.

## Decision

1. **`ProjectRoot.entries`.** The files the root's own manifests say the project runs or ships, relative to the root, existing, with a JavaScript or TypeScript extension, sorted. For an ECMA root, read from the root's and each member's directory:
   - `package.json`'s `main`, `bin` (a path or a map), and the first non-flag argument after `node`, `tsx`, `ts-node`, or `bun` in each script;
   - each `wrangler*.toml`'s top-level `main`;
   - each source a `Dockerfile`'s `COPY` takes from the build context (not `--from` another stage).

   Other ecosystems leave it empty.
2. **An entry roots its module.** `static.dead-code` wants `ProjectInfo`. A module holding an entry file is rooted, as a test file's is, so its symbols are judged by what names them. A function in an entry file that nothing calls is still `dead.symbol` (reach `references`).
3. **`static.dead-code` spans the ECMA family.** It runs once per family and reads `Facts::family` for every kind it reads, joined.

## Consequences

- Molten's `dead.symbol`: 91 → 15. Eight are real dead functions, six are in scripts nothing in the repository names, and one is a script run only from a shell script.
- Dead-code findings for JavaScript and TypeScript are now one family's. A JavaScript module imported only from TypeScript is entered.
- Not yet read, each a remaining false positive class to add when a repository needs it:
  - a file named in a test runner's configuration (Vitest's `globalSetup`, `setupFiles`);
  - a file a shell script or Makefile runs (`node "$DIR/x.mjs"`);
  - another language's Dockerfile `COPY` (a Python module loaded by configuration).
- `ProjectInfo` gains a field. Stored facts without it read as having no entries (`serde(default)`).
