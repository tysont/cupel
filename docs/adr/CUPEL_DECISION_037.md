# CUPEL_DECISION_037: `ProjectInfo` reports detected roots; `init` joins them with the toolchain table

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.9

## Context

Design §8.3.2 lists `ProjectInfo` as a language fact holding "roots, build files, default commands, dirs to skip", and §8.4.1 makes its producer hand-written per language. Design §8.7 puts the default commands' source, the toolchain table, in `cupel-adapt`. Design §8.2 and plan §2.1 forbid `cupel-lang` and `cupel-adapt` from depending on each other, and a crate-graph change needs an ADR (plan §2.5). A language producer therefore cannot read the table, so it cannot fill `ProjectInfo.roots[].commands` (version 1) without duplicating the table. P0.8's detection (ADR-021) also walked every ecosystem at once, skipping the union of their generated directories, and P1.1 left the file walk not honoring `skip_dirs`.

## Options

**Let `cupel-lang` depend on `cupel-adapt`** (or the reverse). The producer could then fill commands, but a language crate would know about tools, the crate graph would change for one join, and P3's tool producers would sit in a crate the language layer links.

**Duplicate the commands in each language's `tables.toml`.** Breaks "adding a tool is one toolchain-table cell" (plan §2.8).

**`ProjectInfo` reports what was detected; the CLI joins it with the table.** The producer reports each root's language, manifests, workspace members, the package patterns its commands cover, and its generated directories. `init` and `doctor`, in `cupel-cli` (which sees every crate), select tools with `cupel-adapt` against each root and fill the commands. The fact loses its `commands` field.

## Decision

- `ProjectInfo` version 2: `ProjectRoot { path, lang, build_files, members, packages, skip_dirs }`. `members` are workspace member directories relative to the root; `packages` are the package patterns commands cover (`./...`, or one `./<module>/...` per `go.work` `use`; empty for other languages). `DefaultCommands` leaves `cupel-core`; `cupel_adapt::Commands` replaces it.
- Each language registers a hand-written `ProjectRoots` producer (`cupel_lang::project`) with its ecosystem from `langs/<id>/project.rs`: manifests, generated directories, and a `describe` that turns a candidate root into a `ProjectRoot` (JavaScript or TypeScript by `tsconfig.json` beside the root's or a member's `package.json`). The walk and the nesting rule are ADR-021 §2's, shared in `cupel_lang::project`; every walk still skips every known ecosystem's generated directories, so results do not depend on which language looks. The producer reads the repository root, not the scope: project layout is repository-wide.
- `init` takes one `ProjectInfo` per registered language from the producer registry, merges the roots (ordered by path, then language), and asks `cupel-adapt` for each root's commands; a language's `[lang.<id>]` table takes its first root's commands, as before. The `init` goldens are unchanged by this.
- `cupel_lang::walk(root, languages, &ProjectInfo)` skips, below each project root, directories named in that root's `skip_dirs`.

## Consequences

- The crate graph stays as designed; the join is the CLI's, which design §8.2 already makes the integration point.
- The design's fact row still says "default commands"; under this ADR they are derived from `ProjectInfo` and the toolchain table rather than stored in the fact. P3's toolchain selection reads the same pair.
- `ProjectInfo` has a conformance inventory (`conformance/project-info.toml`) and fact goldens like every other syntactic fact.
