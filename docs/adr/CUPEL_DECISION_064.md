# CUPEL_DECISION_064: One `checks[]` entry per check, skipped languages folded in

**Date:** 2026-10-14 · **Status:** accepted; supersedes the "followed by one `skipped` entry per skipped language" clause of [041](CUPEL_DECISION_041.md) · **Phase:** P2.10

## Context

ADR-041 gave each check one combined entry in `checks[]` for the languages it ran on, *followed by one `skipped` entry per skipped language*, and no combined entry at all when every language was skipped. The first dogfood run over the Cupel repository (five languages in scope before the fixture repositories were excluded) listed `static.crap` five times (`unconfigured`, once per language) and `tests.weakening` five times (`needs-range`). Consumers key `checks[]` by check ID: the gate takes the first entry with the rule's check, `next_actions` emits one rerun per skipped entry and then de-duplicates them, and an agent reading the envelope sees one check repeated with nothing but the language changing. Splitting by language is also inconsistent with the combined entry, which already hides per-language scores.

## Options

1. Keep ADR-041 and document it. Every multi-language repository repeats each self-skipping check per language.
2. One entry per check always; carry every skipped language in a new list field on `Skipped`. Exact, but a schema change for one rare case (languages skipped for *different* reasons).
3. One entry per check always, using the existing `Skipped` shape: when every language was skipped, one `skipped` entry whose `lang` is null when all languages were skipped alike (they are then exactly `scope.languages`) and the first language's otherwise; when some language ran, the combined entry, made `partial` and carrying the first cut-short language's skip when the timeout or budget skipped a language, and silent about languages the matrix ruled out (those are in the `skip` events and `describe matrix`).

## Decision

Option 3. Per check, in language-ID order:

- **Every language skipped:** one entry, `status: skipped`, whose `skipped` is the first language's. When there are several languages and every skip has the same reason, fact, tool, and `see`, its `lang` is null.
- **Some language ran:** one entry combining the languages that ran (ADR-041's rules for status, score, metrics, findings). When the timeout or budget skipped another language, the status is at least `partial` and `skipped` is that language's skip (so the gate treats the check as incomplete and `next_actions` offers a rerun); otherwise `skipped` is null.
- `skip` events are unchanged: one per ruled-out (check, language).

## Consequences

- `checks[]` has exactly one entry per selected check, so `checks.len()` equals the number of selected checks and lookups by ID are unambiguous.
- A language a check could not run on (unsupported, unavailable, unconfigured, needs a range) while another language ran is visible only in the event stream and `describe matrix`. An all-skipped check whose languages skipped for different reasons reports only the first language's reason.
- Previously a check cut short on one language read as `pass`/`fail`; it now reads as `partial`, which the gate already treats as not completed (ADR-060).
