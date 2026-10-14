# CUPEL_DECISION_061: Ranking, de-duplicating, and capping `next_actions`

**Date:** 2026-10-14 · **Status:** accepted · **Phase:** P2.9

## Context

Design §7.1 shows `next_actions` as a ranked list of `{priority, finding, kind, summary, command}` that mixes finding actions with `rerun` entries carrying a command. The plan asks for ranking by `severity × tier × new_since_baseline`, de-duplication by action kind and location, `rerun` actions for skipped checks, and `install_tool` for unavailable tools. ADR-041 ranked findings by severity then tier, capped at ten, and left the rest to this step. Open: how the three factors combine, what "location" is for findings without one, which skipped checks deserve an action, where non-finding actions rank, which findings are left out, and the cap.

## Options

**Combining the factors.** A product of weights (needs invented weights; a tier-6 `warn` could outrank a tier-0 `fail`) or a lexicographic order. Severity is the check's own judgment of how bad a finding is, so it must dominate. Between tier and novelty, novelty says "you just caused this", which is what an agent in a loop can act on; tier says how strong the evidence is.

**Non-finding actions.** Always last (a gate blocked only by an unevaluated rule would show its remedy below dozens of warnings, or beyond the cap), always first, or banded by what they block.

## Decision

1. **Candidates.** Every finding that is not `suppressed`; every gate rule whose result is `skipped` (`rerun`, command `<verb> <scope flags> --only <check>`) or `unsupported` (`configure`, no command); every skipped check entry by reason: `timeout` or `budget` → `rerun` with the same command form; `unavailable` → `install_tool` with command `cupel doctor`, naming the tool and language; `unconfigured` → `configure` with command `cupel doctor`. `unsupported` skips get no action: the language lacks the fact, and nothing the user runs changes that (`cupel describe matrix` explains it). The scope flags are the run's own (`--changed`, `--base`/`--head`, `--paths`, `--symbols`), so the command reruns the same scope.
2. **Bands.** (1) `fail` findings; (2) actions for gate rules that could not pass; (3) `warn` findings; (4) actions for other skipped checks; (5) `info` findings.
3. **Within a band**, findings new since the baseline first, then higher tier first, then envelope order (file, line, check, kind, ID). Non-finding actions keep the order of `checks[]` and the gate's rules.
4. **De-duplication.** The first action in ranked order wins per key. A finding's key is its action kind plus its location (file, first line, last line), else its symbol hash, else its ID, so two findings on the same function that both ask to `reduce_complexity` give one action. A non-finding action's key is its kind plus the check ID (plus the tool for `install_tool`), so a check skipped for timeout in three languages and also named by a skipped gate rule gives one `rerun`.
5. **Cap.** At most ten actions, numbered `priority` 1..n. The envelope carries every finding, so the list is a starting point, not an inventory; ten keeps a text rendering on one screen.
6. **Scope.** `check` and `survey` compute `next_actions` after suppressions, deltas, and the gate, from the graded envelope. Dry runs and other verbs keep their own (`init`'s and `baseline`'s preview commands).

## Consequences

- `next_actions[0]` is the most severe, newest, best-evidenced finding, or the reason the gate could not be evaluated when no `fail` finding exists.
- A suppressed finding never reappears as an action; an expired suppression's `suppression.expired` warning does.
- Rerun commands assume the binary is invoked as `cupel`, as every other command Cupel prints does.
