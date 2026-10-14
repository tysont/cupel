# CUPEL_DECISION_074: Checks and producers register themselves from their own modules

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P3.0 · **Supersedes:** the registration consequence of [ADR-001](CUPEL_DECISION_001.md)

## Context

ADR-001 made native checks one crate with a module per check, and said registration is "one function in `cupel-checks` that the CLI calls". That function, `cupel_checks::register`, listed every check by hand, so the plan's rule "adding a check is one module plus goldens" (§2.8) was really one module, one line in `register`, and edits to every test, golden, and snapshot that enumerated checks. The same held for producers: `cupel-lang` attached its generic producers from one table (`GENERIC` in `generic.rs`), and `cupel-adapt`, where P3's runner and coverage producers go, had no registration path at all, so the first tool producer would have had to edit the CLI's catalog and the matrix harness.

P3 fans out to parallel worktrees, each adding a check or a producer. Every shared list is a line two branches edit at once, which is a merge conflict. The question is how a check or producer joins its registry from its own file.

## Options

**`inventory` (`inventory::submit!` in each module, `inventory::iter` in `register`).** Each submission is a static the linker collects through a platform constructor section; iteration order is unspecified, so `register` sorts. Works on the pinned stable toolchain on macOS and Linux, under LTO, and (checked before choosing) under the workspace's `unsafe_code = "forbid"`: the macro's unsafe code is expanded from the `inventory` crate, which the lint does not attribute to the caller. A small dependency (no transitive dependencies) from a well-known author. A submission in a test binary joins that binary's registry, which is exactly what a duplicate-ID test needs.

**`linkme` (`#[distributed_slice]`).** The same idea with a link-section slice instead of constructors, so no code runs before `main`. Its attribute expands to `#[link_section]` statics declared at the use site, which the workspace's `unsafe_code = "forbid"` rejects (checked on 1.92: "declaration of a static with `link_section`"), and `forbid` cannot be relaxed per crate. No concrete advantage here outweighs that.

**A build script that globs `src/*.rs` and generates the module declarations and the registration list into `OUT_DIR`.** No runtime mechanism and no `mod` line to share. But modules declared through `include!` are invisible to `rustfmt` (checked: `cargo fmt --check` does not visit them), so every check would fall outside `make lint`'s format gate, and the build script needs file-system access in the one crate whose clippy configuration forbids it.

**Keep the lists.** Every parallel P3 step conflicts on the same lines; resolution is mechanical but constant.

## Decision

`inventory`, in three places, each with its own submission type and one public entry point:

- `cupel-checks`: `NativeCheck(fn() -> Box<dyn Check>)`, submitted by each check module (`inventory::submit!(NativeCheck(|| Box::new(StaticClones::new())))`). `register(&mut CheckRegistry)` builds every submission, sorts by check ID (the order `describe` and the envelope use), and panics with "native check IDs are unique" on a duplicate, which a test that submits a second `static.complexity` proves.
- `cupel-lang`: `GenericProducer { kind, drives, attach }`, submitted by each fact module (`symbols`, `decisions`, `normalize`, `mutations`, `testframes`, `conventions`, `api`, `roots`). `generic::register` iterates them in fact-catalog order. Language-specific producers stay in the language's own `register()` in `langs/<id>/mod.rs`, which is already one file per language; the language list in `LanguageRegistry::builtin` stays a list, because adding a language is a cross-cutting checklist (design §8.4.4) that never runs in parallel.
- `cupel-adapt`: `ToolProducer { id, register }`, submitted by each tool producer module. `cupel_adapt::register(&mut ProducerRegistry)` applies them in ID order and panics on a duplicate ID. The CLI's catalog and the matrix harness call it now, with no submissions yet, so `cupel-fixtures` depends on `cupel-adapt` (a crate-graph change: the harness must see the producers the binary sees, or the matrix it computes would disagree with `describe matrix`). Preference among producers of one fact is the toolchain table's job (P3.1), not registration order.

The rule: **a new check adds its module file, its `mod <name>;` line in `lib.rs`, its fixtures, and `golden/<check-id>/`, and edits nothing else; a new tool producer adds its module file and its `mod` line.** The `mod` line cannot be avoided without the build-script option's format-gate hole, so it is the one shared line, and `.gitattributes` gives `crates/cupel-checks/src/lib.rs`, `crates/cupel-adapt/src/lib.rs`, and `crates/cupel-lang/src/lib.rs` git's built-in `union` merge driver: two branches that each add a `mod` line merge cleanly with both lines (checked), and `make fmt` restores the order if they land out of it. A test in `cupel-checks` reads its own `lib.rs` (through `include_str!`) and fails if it names any check type, so the rule cannot erode.

## Consequences

- `register()` is still the one call that adds every native check; ADR-001's crate granularity and split criterion are unchanged.
- A check module that forgets `inventory::submit!` is simply not registered; the describe-coverage and matrix tests derive their expectations from the registry, so they cannot catch an unregistered check — but nothing else references it either, and `dead_code` warns about its unused type, which `make lint` turns into an error.
- Registration happens through linker sections, so a crate whose items are never referenced could in principle be dropped by the linker. Every crate here is referenced through its `register` entry point, which keeps its object files and their submissions in the link.
- `union` applies to every conflicting hunk in those three files, not only `mod` lines; two branches that edit the same doc paragraph would keep both versions. Those files are otherwise rarely edited, and the duplicate would be visible in review and usually fail to compile.
- Goldens and tests that rendered every check at once move to per-check files under `golden/<check-id>/` or derive their expectations from the registries (plan P3.0); the README's generated tables are the one remaining whole-registry artifact, regenerated with `make readme` after a merge.
