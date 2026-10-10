# CUPEL_DECISION_011: Language identity is an open string type in `cupel-core`

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.2

## Context

P0.2 defines `Langs` (`Any | Only(&[LangId]) | Family(LangFamily)`), `Producer::languages`, and `SupportMatrix::compute(checks, producers, langs)` in `cupel-core`. All of them name `LangId` and `LangFamily`, yet plan P1.1 lists those two types under `cupel-lang`. `cupel-core` cannot depend on `cupel-lang` (design §8.2), so the types must live in core. Their shape matters as much as their home: plan §2.8 says adding a language must not touch `cupel-core`.

## Options

**Closed enums in `cupel-core`** (`LangId::Rust`, `LangFamily::Ecma`, …). Exhaustive matching and typo-proof literals, but every new language (Java and Clojure in P6) edits `cupel-core`, which §2.8 and design §8.4.4 classify as a defect in the extension model. A closed enum also invites `match lang` in places that must stay language-blind.

**Open string newtypes in `cupel-core`** (`LangId("rust")`, `LangFamily("ecma")`). Core knows that languages exist and how to compare them, never which ones exist. Languages are named once, in `cupel-lang`, where each `Language` impl returns its own id and family. Typos are caught by the registry rather than the compiler.

**Generic parameter over a language type.** Makes every core type generic for no benefit; the language set is a runtime registry anyway.

## Decision

`LangId` and `LangFamily` are serde-transparent string newtypes in `cupel-core`. A `Lang { id, family }` pair is what the support matrix and `Langs::covers` consume; `cupel-lang` builds those pairs from its `Language` registry. `cupel-core` contains no language names outside tests.

## Consequences

- Adding a language never edits `cupel-core`, matching design §8.4.4.
- P1.1 implements the `Language` trait and registry in `cupel-lang` using these core types instead of defining its own.
- Validation of a language name (for `[lang.<id>]` config keys, `describe language <id>`) is a registry lookup with a usage error, not a parse failure.
