# CUPEL_DECISION_112: A language registers once: `Language::ecosystem`, and the registry hands every project-root walk the ecosystems it collects

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P6

## Context

ADR-110 (P6.4, Java and Clojure) recorded an extension-model defect and left its fix for the P6 merge: `crates/cupel-lang/src/langs/ecosystems.rs` held `KNOWN`, a hand-written list naming every language's `Ecosystem` (its manifests, generated directories, and how a root is described), which every `ProjectInfo` walk skips the generated directories of. A language was registered twice: its line in `langs/mod.rs` and its entry in `KNOWN`; a language added to the first and forgotten in the second would have its build output (`target/`, `build/`) walked by every other language and reported as project roots. Each of the six language modules also registered its own `ProjectRoots` producer with the same five lines (`ProjectRoots::new(language, &project::ECOSYSTEM, &super::ecosystems::KNOWN)`). ADR-110 proposed that `LanguageRegistry` collect each language's ecosystem, through a `Language::ecosystem()` or a registration-time argument, and noted that either changes the closed `Language` trait or the `ProjectRoots` constructor.

`ecosystems.rs` also holds ADR-038's `UNREGISTERED` (ecosystems recognized before their language is registered, whose roots `init` reports through `cupel_lang::unregistered_roots`); it is empty since P6.4 and stays for the next language detected before it is registered.

## Options

**A registration-time argument.** `Language::register(&self, reg, known: &[&Ecosystem])`: the registry computes the list and passes it in, each language still registers its own `ProjectRoots`. The trait's signature changes for every language, the registry cannot compute the list without asking each language for its ecosystem anyway, and the six copies of the registration stay.

**`Language::ecosystem() -> Option<&'static Ecosystem>`, with the language registering `ProjectRoots`.** The registry would collect the list and hand it to languages somehow (a global, or the argument above): both mechanisms at once.

**`Language::ecosystem() -> Option<&'static Ecosystem>`, with the registry registering `ProjectRoots`.** A language names its ecosystem; `LanguageRegistry::register_producers` collects every language's ecosystem (and `UNREGISTERED`'s) and, for each language that has one, registers its `ProjectRoots` with the full list. One trait method, required (a language without build files says `None`), and the six copies disappear. The design's trait (§8.4.1) is "deliberately small and closed" against check-driven growth; this method is not knowledge a check needs but how the language is registered, as `register` is. Chosen.

## Decision

1. `Language` has `fn ecosystem(&self) -> Option<&'static Ecosystem>`. Each built-in language returns `Some(&project::ECOSYSTEM)` from its own `langs/<dir>/project.rs` (JavaScript and TypeScript share `ecma`'s), and no language module registers `ProjectInfo` any more; Clojure's `resolve::register` registers only its module graph.
2. `LanguageRegistry::register_producers` registers each language's producers, then, for a language with an ecosystem, `ProjectRoots::new(language, ecosystem, known)`, where `known` is `ecosystems::known(registry)`: every registered language's ecosystem in ID order (one shared by two languages appears twice, which the walk's "any ecosystem generates this directory" test does not mind), then `UNREGISTERED`. `ProjectRoots` holds its list as a `Vec<&'static Ecosystem>`.
3. `KNOWN` is gone. `UNREGISTERED` and `unregistered_roots` stay as they were (ADR-038); `unregistered_roots` walks with `known(&LanguageRegistry::builtin())`, so its walks skip the same directories. `langs/mod.rs`'s `with_cache` list is the one place a language is registered.
4. `crates/cupel-lang/tests/project.rs` gains `every_walk_skips_the_generated_directories_of_every_registered_ecosystem`: from the registry alone, it places a manifest of every registered ecosystem under every visible generated directory of every registered ecosystem, and under a control directory, and requires that the registered languages' roots are exactly the control ones. With the list emptied, the roots under `build/`, `dist/`, `coverage/`, `venv/`, and `__pycache__/` appear and the test fails (`target/`, `vendor/`, and `node_modules/` are skipped regardless, by `Exclude`'s defaults).

## Consequences

- A language is added by its `langs/<id>/` directory and its line in `langs/mod.rs`; its ecosystem reaches every walk without another edit, and its `ProjectInfo` producer is registered for it.
- The `Language` trait has one more required method. A language with no build files (none today) returns `None` and gets no `ProjectInfo` producer, which the support matrix then shows.
- `ProjectInfo` producers are registered after each language's own producers rather than among them; each kind keeps one producer per language, so no candidate order or golden changed.
- Design §8.4.4's checklist says a language's `register()` hand-writes `ProjectInfo`; it now names its ecosystem instead, and the registry registers the producer.
