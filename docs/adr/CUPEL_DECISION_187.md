# CUPEL_DECISION_187: Enabling the `strings` operator group, and where its sites are

**Date:** 2026-11-11 · **Status:** accepted · **Phase:** v0.2.7, item 8 · Details [180](CUPEL_DECISION_180.md)

## Context
ADR-180 put a `strings` operator group in the extended set, off unless enabled by explicit configuration, and not turned on by `--depth deep`. It did not say how a user enables it, whether it can run without the rest of the extended set, or which strings are sites.

## Options
**Spelling.** (a) A third value, `operators = "strings"`: reads as a set beside `default` and `extended`, which the group is not. (b) A list, `groups = ["strings"]`: room for groups that do not exist. (c) A boolean beside the set, `strings = true`.

**Without the extended set.** Enabling the group alone would make it a set of its own, against ADR-180's "joins the extended set"; ignoring it silently would hide a configuration mistake.

**Which strings.** Every string literal would mutate type annotations (TypeScript literal types, which tests do not execute, so their mutants survive), object and dictionary keys, imports, and docstrings.

## Decision
- `[lang.<id>] mutation = { operators = "extended", strings = true }` enables the group. `strings = true` with any other set is a configuration error naming `mutation.strings`; `strings` is a boolean, `false` when absent.
- The group's operators are `string-empty` (a non-empty string literal to `""`), `regex-quantifier`, and `regex-class`. Their sites are in the extended set (`set: "extended"` in `mutation-sites`) and are always extracted; the engine plans them only when the group is enabled, at every depth.
- A string is a site where it is a value: a variable's or assignment's value, an argument, an element, a dictionary or object value, a conditional's branch, a binary operand, a returned value. Types, keys, imports, and docstrings are not sites. Empty strings (with any prefix) are not sites.
- A regex literal's pattern is a site of both regex operators. Their replacements are computed from the pattern, one mutant per edit: each quantifier outside a class (`*`↔`+`, a `?` removed unless it opens group syntax, `{n,m}` or `{n,}` to `{n}`), each negated class un-negated, each class range narrowed to its first member.
- JavaScript and TypeScript capture strings and regex literals; Python captures strings (it has no regex literal). Other languages capture neither until they ask for it.

## Consequences
Default and `extended` mutation runs, at any depth, plan the same mutants as before; `mutation-sites` facts carry more sites. A language gains the group by adding `@mut.string-empty` (with a `[mutations.string-empty]` table) or the regex captures to its `mutations.scm`.
