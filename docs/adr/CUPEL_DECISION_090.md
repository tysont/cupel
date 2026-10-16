# CUPEL_DECISION_090: Oracles: where they are defined, what `verify` does per kind, the identity hash, and the store

**Date:** 2026-10-16 · **Status:** accepted · **Phase:** P5.2

## Context

Design §6.5: `cupel oracle add NAME --kind ref|command|spec --ref REF | --cmd "…" | --spec FILE --lang tla+|quint|p`; `verify NAME` "runs a smoke comparison (ref/command: same output on N known inputs; spec: parses and model-checks a trivial behavior) and records `verified_at` + hash in the fact store". §6.7 shows oracles in `cupel.toml` (`[oracles.pg] kind = "command"`, `cmd = "psql -X -A -t -c {input}"`); §6.8 lists `oracle add` among the verbs whose dry run carries a unified diff per file; §8.8 has an `oracles (name PK, kind, spec JSON, verified_at, verified_hash)` table. `cupel.toml` already parses `[oracles.<name>]` (`OracleConfig`, P0.4), and migration 1 created the table, which nothing has written.

The design is silent on which of the two homes is the source of truth, what a smoke comparison is before any check can run a program against generated inputs (input generators and the differential harness are P5.3), what the hash covers, when a verification stops counting, and which exit codes a failed verification has.

## Options

**Home of the definition.** (a) The store alone: simple, but `.cupel/` is not committed, so every clone and CI runner would re-add every oracle, and `[oracles]` in `cupel.toml` would be a second, competing home. (b) `cupel.toml`, with the store holding only verification: definitions are reviewed and shared like the rest of the configuration, `oracle add` has a file diff as §6.8 says, and a verification is local evidence, like a run record. (c) A separate committed file under `.cupel/` (as suppressions are): a third configuration file for one table.

**Editing `cupel.toml`.** Appending text works for `add`, but `remove` cannot find a hand-written table reliably; re-serializing the parsed file loses comments and ordering. `toml_edit` (the editing half of the `toml` crate family already in the build) changes one table and keeps everything else byte for byte.

**Smoke comparison before P5.3.** A full "same output as the implementation on N inputs" needs the harness that runs the implementation, which P5.3 builds. What can be checked now, per kind, is that the oracle exists and behaves as an oracle must: it resolves, it runs, and it answers the same question the same way twice.

**Expiry.** By age (a verification older than N days lapses), or by identity (a verification is for one exact oracle and lapses when that oracle changes). Age would expire a pinned tag that cannot change and keep a branch that moved yesterday.

## Decision

1. **Definitions live in `cupel.toml`** (`[oracles.<name>]`, the existing `OracleConfig`), the source of truth. Names match `[a-z][a-z0-9_-]*`. Each kind takes exactly its keys, checked when the file loads (a usage error, exit 2, naming the table) and by `add`:
   - `ref`: `ref` (a git ref or commit).
   - `command`: `cmd` (run by `sh -c` in the repository root; `{input}` is replaced by the shell-quoted input), and optional `inputs`, the known inputs of the smoke comparison (a new optional key; default one empty input).
   - `spec`: `spec` (a path relative to the repository root) and `lang` (`tla+`, `quint`, or `p`).

2. **`oracle add` and `oracle remove` change `cupel.toml`** with `toml_edit` (a new workspace dependency, `cupel-cli` only), through the `Writer`: like `init` and `baseline set`, without `--write` they preview (`status: plan`, `would_write` with the file's diff, a `next_actions` entry that reruns with `--write`), and `--dry-run` does the same. The file edited is the configuration that loaded (`--config` included), else `<root>/cupel.toml`, created if absent. `add` refuses a name already defined and `remove` an undefined one (exit 2, defined names listed). `remove --write` also deletes the oracle's verification. `add` takes `--input VALUE` (repeatable) for a command's known inputs.

3. **`oracle verify NAME | --all`** runs the smoke comparison of each named oracle, writes nothing outside `.cupel/`, and needs no `--write`; its dry run runs nothing and plans nothing.
   - **ref:** the ref resolves to a commit (`git rev-parse --verify REF^{commit}`). Comparing the reference build's outputs with the change's is `gen.differential`'s (P5.3).
   - **command:** for each known input, the command runs twice; both runs must exit 0 and print the same standard output. A deterministic, runnable oracle is the precondition of every differential comparison.
   - **spec:** the file exists and the language's own parser accepts it: `quint parse FILE`, `sany FILE` (TLA+), `p compile --pfiles FILE` (P). Model-checking a trivial behavior waits for the formal checks (P7), which own the model checkers' invocation.

   Outcomes, per oracle: verified (recorded; exit 0); the oracle cannot be reached — not a git repository, an unknown ref, a missing spec file, a parser or command program that is not installed (`sh` exit 127) — `oracle.unreachable`, severity `fail`, exit 3, action `configure` or `install_tool`; the comparison disagrees — a non-zero exit, two different outputs, a spec that does not parse — `oracle.smoke-failed`, severity `fail`, exit 1, action `verify_oracle`, with the input and the outputs (or the parser's output) in evidence. Exit 3 outranks exit 1 under `--all`. Any failure deletes the oracle's previous verification: an oracle that fails its smoke comparison is not trusted on the strength of an older one. An unknown name is exit 2; `--all` with no oracles passes with nothing verified. Findings are subject findings (ADR-016) of check `oracle` whose subject is the oracle's name.

4. **The identity hash.** BLAKE3 (hex) of the canonical JSON `{"version": 1, "definition": <the OracleConfig>, "resolved": R}`, where `R` is the commit the ref resolves to (`ref`), the BLAKE3 of the spec file's bytes (`spec`), or null (`command`). It is computed by `verify` and recorded; it is computed again wherever verification is asked about (`oracle list`, every `check` that ran against the oracle) from the configuration and the repository as they are.

5. **Expiry is by identity, not by age.** An oracle is `verified` when the store has a verification whose hash equals its identity now; `unverified` when it has none; `changed` when the recorded hash differs (the definition was edited, the ref moved, the spec file changed); `unreachable` when the identity cannot be computed now (the ref or spec file is gone). A failed `verify` and `remove --write` delete the verification. A command oracle's identity is its definition only: a different program behind the same command line keeps its verification (consequences).

6. **The store, migration 5.** The version-1 `oracles` table was never written; migration 5 drops it and creates `oracles (name TEXT PRIMARY KEY, kind TEXT NOT NULL, spec TEXT NOT NULL, verified_at TEXT NOT NULL, verified_hash TEXT NOT NULL, run_id TEXT NOT NULL)`: a row exists only for a verified oracle, so its columns are required; `spec` is the definition verified (JSON); `run_id` is the `oracle verify` run that recorded it, whose run record holds the smoke comparison's envelope. `Store::{record_oracle, verified_oracle, verified_oracles, forget_oracle}`.

## Consequences

- An oracle defined in a committed `cupel.toml` is unverified in every fresh clone until `cupel oracle verify` runs there: a verification is evidence about this machine's tools and this repository's refs.
- `doctor`'s "oracle reachability (`oracle verify --all`)" (design §6.5) is not wired in this step; `verify --all` is the command.
- P5.3 replaces the `ref` and `command` smoke comparisons with "same output as the implementation on N known inputs" when its harness exists; the identity hash and its version stay unless what it covers changes (bump `version` then).
- A command oracle whose program is upgraded keeps its verification until it is verified again or its definition changes. Hashing the program's binary was rejected: commands are shell lines, often pipelines, and their programs cannot be named reliably.
- The spec parsers are named by their usual command-line names; an installation that names them differently is `oracle.unreachable` with an `install_tool` action until P7 makes them configurable.
