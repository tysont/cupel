# CUPEL_DECISION_196: A `node:test` root's coverage can come from Node's own LCOV reporter

**Date:** 2026-11-13 · **Status:** accepted; amended by [197](CUPEL_DECISION_197.md) (owner rulings, 2026-11-13) · **Phase:** v0.2.7, item 6 · Details [189](CUPEL_DECISION_189.md), [079](CUPEL_DECISION_079.md)

## Context
ADR-189 gave a `node:test` root test results, but its coverage still needed c8, the only coverage producer the toolchain table paired with `tool.node-test`. Molten's `containers/sandbox` has no c8: with `[lang.javascript] coverage` set to Node's own command (`node --test --experimental-test-coverage --test-reporter=lcov --test-reporter-destination={out}`), `tests.coverage` still skipped with "c8 unavailable", because the configured command is the language's and c8's availability asks only whether c8 is installed. Node writes LCOV itself from 22 on, with no package; its report lists the files the suite loaded and leaves test files out.

## Options
(a) A second coverage producer, `tool.node-test-coverage`, that needs only `node` and runs the configured command. (b) Make c8's producer run any command when c8 is missing: it would report Node's coverage under c8's ID. (c) Change `init` to write Node's command for `node --test` roots: a default change the owner has not ruled on.

## Decision
- `tool.node-test-coverage` pairs with `tool.node-test` after c8 in the table. It needs `node`, reads LCOV, and lists only loaded files (ADR-152).
- Where two coverage producers pair with one runner, each runs only a command that uses it: a spec may name a word its command must contain (`c8` for c8, `--experimental-test-coverage` for Node's). A configured command without it leaves that producer unconfigured, so the run falls through to the producer the command is for.
- `init` still writes c8's command for a `node --test` root; a root without c8 configures Node's command. Whether `init` should prefer Node's own coverage is left to the owner.

## Consequences
A `node:test` root without c8 gets coverage, and the checks that need it, from its configured Node command. c8 no longer runs a command that does not name it. Node counts every line of a loaded file in LF, comments included, so a file's instrumented line count differs from c8's or Jest's for the same source.
