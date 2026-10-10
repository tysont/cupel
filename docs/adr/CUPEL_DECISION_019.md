# CUPEL_DECISION_019: JSONL header shape, paging semantics, and text rendering rules

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.5

## Context

Design §7.1 says `jsonl` "emits one finding per line after a header line", and the plan says the header is the "envelope minus `findings`". Design §6.1 rule 8 requires `--limit/--offset` paging with `truncated` and `total_findings`, and §7.3 describes the text rendering in one paragraph. Building the renderers in P0.5 needs answers none of these give:

1. Whether the JSONL header omits the `findings` key or carries it empty.
2. What paging counts, in which order it cuts, and what else in the envelope it changes.
3. What "no information present in JSON is omitted except evidence bodies" means for `null`s, empty lists, open maps such as `config.effective` and `action.hints`, and the `describe` payload; and how evidence is "summarized to one line".
4. When the text renderer uses color, and how much.

## Options

For (1): omitting `findings` matches "envelope minus findings" literally, but the header is then the only Cupel object without the fixed key set of ADR-013 and cannot be parsed as an `Envelope`. Emitting `"findings": []` keeps the key set, keeps the header a valid envelope, and makes reassembly a single assignment; its cost is that a reader looking only at the header could take `[]` for "no findings", which `total_findings` contradicts.

For (2): paging could re-derive `summary` and `checks[].findings` from the page, or leave them describing the run. Re-deriving makes the summary disagree with the gate and the exit code, which are computed from every finding.

For (3): printing every key, including `null`s and empty collections, is literal but buries the signal; printing only "important" fields omits information. In between: a value that is `null` or an empty collection carries no information, so it is skipped; everything else is printed.

For (4): color always on a terminal, or a configurable palette, or the common conventions — off for a pipe, off with `--no-color`, off when `NO_COLOR` is set (no-color.org).

## Decision

1. The JSONL header is the complete envelope serialized on one line with `"findings": []`; each following line is one finding of the page, in envelope order, serialized on one line. Every line ends with `\n`. Replacing the header's `findings` with the parsed lines yields exactly the `json` envelope.
2. Paging applies to `findings` only, in envelope order, after the verb runs and before any renderer, for every format and every envelope (including error envelopes): skip `--offset` findings (default 0), then keep at most `--limit` (default: no limit). `total_findings` is the number of findings before paging, and `truncated` is true exactly when the page holds fewer findings than `total_findings`. `summary`, `checks`, `next_actions`, `status`, and `exit_code` describe the whole run and are never changed by paging. An offset beyond the end yields an empty page.
3. The text renderer prints every present value of the envelope; a `null` or an empty list or map is absent and is not printed. Open maps (`config.effective`, `action.hints`, `toolchain`) are printed key by key with values as compact JSON. Evidence is the one exception: it is summarized on one line as its keys, with scalars shown inline (strings shortened to 40 characters) and lists and maps shown by size, followed by a pointer to `cupel explain <id>`. The `describe` payload is rendered as readable sections (verbs, flags with value names, environment variables, defaults, values, conflicts, pending steps; exit codes; schemas), and a schema body is printed as indented JSON. Findings are grouped by severity (`fail`, `warn`, `info`), then by check in order of first appearance, keeping envelope order within a group. `next_actions` is always the last section. When the page is truncated, a line before the findings says how many of `total_findings` are shown and that `--offset`/`--limit` page through them. Stability is printed for the CLI as a whole and, for any verb, flag, exit code, or schema, only where it differs from the CLI's.
4. Text uses color only when stdout is a terminal, `--no-color` is absent, and `NO_COLOR` is unset or empty. Color is limited to ANSI SGR codes for the status word and gate result (green pass, red fail and error, yellow partial, cyan plan), severity headings (red, yellow, blue), bold section headings, and dim secondary lines (`repro`, `suppress`). No other formatting depends on color, so output without color carries the same information.

## Consequences

- JSONL consumers can parse the header with the same `Envelope` type as JSON and must use `total_findings` and `truncated`, not the header's `findings`, to learn how many findings exist.
- Agents page through large runs with `--offset` while the summary, gate, and exit code stay stable from page to page.
- The text renderer's snapshot tests pin this layout; changing what is printed is a reviewed snapshot change.
- SARIF (P6.1) is added beside these renderers in `cupel-cli/src/render.rs` and receives the paged envelope like the others.
