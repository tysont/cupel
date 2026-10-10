# CUPEL_DECISION_003: Golden envelope redaction, storage, update, and diff reporting

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P0.6

## Context

Design §11 says fixture tests "assert envelope equality modulo `run.id`, timestamps, durations", and plan P0.6 extends the list to toolchain versions, absolute paths, and `git.sha`. Neither document says exactly which envelope fields vary between two runs of the same command on the same input, what a redacted value looks like, how a golden is stored so its diff is readable, how goldens are rewritten, or what a failure prints. From P2 on the matrix test (design §8.3.7) compares one golden per supported `(check, lang)` pair, so these answers are applied hundreds of times and must be settled before the first golden is written.

## Options

**Compare only the fields a test names.** Each test asserts the fields it cares about. No redaction rules, but nothing catches an unintended change anywhere else in the envelope, which is the point of a golden.

**Normalize by rewriting the envelope type in place.** Redact by mutating a typed `Envelope` (zero durations, fixed IDs). Keeps types, but a placeholder must then be a valid value of the field's type (a fake ULID, a fake timestamp), which reads as real data in a golden and can collide with real values.

**Redact into JSON with explicit placeholders.** Serialize the envelope to a `serde_json::Value` and replace each volatile value with a string placeholder such as `"<run-id>"`. Placeholders are unmistakable in a diff and cannot be confused with real output; the cost is that the golden is not itself a valid envelope.

For the diff, `pretty_assertions` renders a coloured side-by-side diff inside `assert_eq!`, but it cannot add the golden path or the update command, and its colours garble CI logs. `similar` produces a plain unified diff with caller-chosen headers and is the diff engine `insta` already uses. Pinning `similar` 2.7 (the version `insta` resolves to) rather than the newest 3.x adds no new code to the build.

## Decision

1. **Redaction produces JSON.** `cupel_fixtures::redact(envelope, root)` serializes the envelope and replaces volatile values with placeholder strings. A `null` is never replaced, so ADR-013's fixed key sets and the meaning of `null` survive redaction; only present values are masked.
2. **The volatile fields and their placeholders:**

   | Field | Placeholder | Why it varies |
   |---|---|---|
   | `cupel.version`, `description.version` (`cli`), `description.cli.version` (`all`) | `<version>` | changes every release |
   | `run.id`, `run.baseline.id` | `<run-id>` | fresh ULID per run |
   | `run.started` | `<timestamp>` | wall clock |
   | every `duration_ms`, at any depth | `<duration-ms>` | wall clock |
   | `run.seed` | `<seed>` | random unless `--seed` is given (P0.7) |
   | every value of `run.toolchain` (keys kept) | `<version>` | the machine's installed tools |
   | `run.git.sha`, `run.git.base` | `<sha>` | the commit the fixture happens to sit on |
   | `run.git.branch` | `<branch>` | CI checks out detached heads, developers do not |
   | `run.git.dirty` | `<dirty>` | the enclosing checkout's state |
   | the repository root inside any string | `<root>` | where the checkout lives |

   The root is replaced as both the path given and its canonical form (macOS temp directories live under a `/var` → `/private/var` symlink), longest first. Exit-6 envelopes, whose `bug_report` evidence carries the OS and architecture, are not golden-tested; their tests assert fields directly.
3. **Storage.** A golden named `a/b` lives at `crates/cupel-fixtures/golden/a/b.json`; names are relative, slash-separated, without an extension or `..`. `cli/describe` today; `<check>/<lang>` for the matrix from P2. The file is the redacted JSON pretty-printed with two-space indentation, object keys sorted recursively, and a trailing newline. Sorting makes the bytes independent of serde_json's `preserve_order` feature, which feature unification may switch on in some builds and not others.
4. **Comparison** is equality of parsed JSON values, so a hand-edited golden with different whitespace or key order still passes; the bytes on disk are canonical whenever the harness writes them.
5. **Updating.** `UPDATE_GOLDENS=1 make test` rewrites every golden that differs or is missing and passes; any other value, or no value, compares. CI never sets it. `make test` passes the environment through to `cargo test` unchanged.
6. **Failure messages.** A mismatch panics with the golden's path, a unified diff rendered by `similar` (three lines of context) from the golden (`--- <path>`) to the actual output (`+++ actual`), and the line `run UPDATE_GOLDENS=1 make test to accept the actual output`. A missing golden panics with its path, the same update command, and the actual redacted JSON.
7. **Invoking the binary.** `run_cupel(args, cwd)` runs the `cupel` binary that Cargo built into the target directory (`assert_cmd`'s lookup), with stdin closed and every `CUPEL_*` variable removed from its environment, and parses stdout as an `Envelope` when it is a JSON object; JSON that is not an envelope panics, since it breaks the output contract. `make test` (`cargo test --workspace`) always rebuilds the binary first; running `cargo test -p cupel-fixtures` alone can use a stale binary, which is one more reason the Makefile is the only entry point (plan §2.3).

## Consequences

- A golden diff shows exactly the contract change and nothing ambient; a new volatile field shows up as a failing golden on the second machine, and is fixed by adding a row here (a new ADR) and a rule in `redact`, never by loosening comparison.
- Tests that need a volatile value (seed echo, git state, timing) assert it on the typed envelope directly.
- Goldens for runs that find things should pass `--seed`, so `repro` strings, which embed the seed, are stable.
- Configuration discovery walks up from the working directory (P0.4), so a golden run must happen in a directory whose ancestors hold no `cupel.toml` the test did not put there: the CLI goldens run in a fresh temporary directory, and fixture repositories (P1) carry their own `cupel.toml` so discovery stops inside them.
