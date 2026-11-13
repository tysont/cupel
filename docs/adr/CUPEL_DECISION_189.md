# CUPEL_DECISION_189: The `node --test` runner reads Node's JUnit report, from Node 24.11.1

**Date:** 2026-11-12 · **Status:** accepted, detailed by [196](CUPEL_DECISION_196.md) · **Phase:** v0.2.7, item 6 · Details [075](CUPEL_DECISION_075.md), [139](CUPEL_DECISION_139.md)

## Context
The toolchain table lists `tool.node-test` (`node --test`) as JavaScript's and TypeScript's runner when no Vitest or Jest is the root's, and `init` writes it, but no producer ran it or read its results. A `node:test` root (Molten's `containers/sandbox`) got no `test-results`, so every check that needs them skipped. Node writes JUnit XML with `--test-reporter=junit --test-reporter-destination=<file>`, which the JUnit parser reads. Before 24.11.1, Node's JUnit report does not say which file a test is in: a top-level test is a bare `<testcase>`, its `classname` the constant `test`. Node 22, which CI and `make dev-setup` pinned (ADR-139), writes that form.

## Options
**The report.** (a) Node's JUnit reporter: a parser exists; no file before 24.11.1. (b) A reporter module Cupel writes under `.cupel/` and passes as `--test-reporter=<path>`: Node's test events carry the file in releases before 24.11.1 too, at the cost of JavaScript Cupel owns and must keep in step with Node's event shapes. (c) Node's TAP or spec output: no file either, and a format with no other reader.

**IDs without a file.** A test identified by its suites and name alone collides across files and cannot be narrowed by file.

## Decision
- `tool.node-test` runs `node --test --test-reporter=junit --test-reporter-destination=<report>` in each root and reads the report with `junit::node`. A test's ID is the Jest form (ADR-075): `<file> > <suites> > <name>`, the file relative to the root, the suites the `<testsuite>`s it is nested in (Node's `describe`s). A file that does not load is one test Node names after the file: its ID is the file, `errored`.
- Node 24.11.1 or later is what the runner expects. An older Node's report still reads, each test identified by its suites and name, with no file; nothing refuses it.
- CI's and `make dev-setup`'s Node pin moves from 22.23.3 to 24.22.3, the current 24 LTS release.
- Test files narrow a run (`node --test <files>`); no test can be left out. Node has no shuffle or seed: a run's parallelism is `--test-concurrency`, and an ordering request leaves Node's order (files in parallel processes, tests in declared order). The flake engine's reruns therefore differ only in concurrency.
- `tool.node-test` has no detection rule, as the table always had it: it is detected in every root, so it is chosen before an undetected Vitest or Jest, as `init` already chose it, and after a detected one.

## Consequences
A `node:test` root gets test results, `tests.flaky`, and the baseline. Its coverage still comes from c8 (B17). A repository on Node older than 24.11.1 gets test results without files, so file-based narrowing and per-file attribution do not apply to it.
