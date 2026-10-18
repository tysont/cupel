# CUPEL_DECISION_119: `conc.linearizability`: the `Histories` payload, histories a project's command records, and Porcupine as the checker

**Date:** 2026-10-17 · **Status:** accepted · **Phase:** P7.3

## Context

Design §5.4: `conc.linearizability` "record[s] concurrent op histories, check[s] with Elle/Porcupine against a model", kind `consistency.violation` "(anomaly type, history excerpt)", cost deep/exhaustive, wrap, tier 5; §5.8 runs it at `deep`. §8.3.2 names `Histories` among the facts P7 adds, so the kind needs no ADR of its own, but plan P7 asks for its schema and conformance inventory before its first producer. Nothing says how a history gets recorded (Cupel cannot instrument a system it does not know), what format it has, how the model is chosen, which checker runs it and how Cupel runs a checker that is a Go library (Porcupine) or a Clojure library (Elle), what a violation's excerpt holds, or how a check that never sees a language becomes supported for languages whose code records the history.

## Options

**Who records the history.** (a) Cupel instruments the system's client calls: language-specific, and the calls that matter (a key-value client, a queue) are the project's own. (b) The project's tests or a harness record it, and the project names the command in configuration; Cupel runs the command and checks what it wrote. (b): the producer wraps the checker, not the test, and works for any language a project writes its harness in.

**The format.** Jepsen's history model is the de facto one: per process, an `invoke` followed by an `ok` (it happened), `fail` (it did not), or `info` (unknown: a timeout, a crash) completion, each with a function `f` and a `value`. As JSON lines it is written by any language without a library, appends while the harness runs, and reads one line at a time; its first line declares the model, so one command can record histories of several models. Real time is the order of the lines; no clock is needed.

**The checker.** Porcupine is a Go library (no binary), checks linearizability of single-object histories against a model written in Go, partitions by key, and reports the longest linearizable prefixes it found, which make an excerpt. Elle is a Clojure library that checks transactional histories (list-append, rw-register) for isolation anomalies (G0, G1c, G2…), on the JVM, its dependencies resolved from Clojars. Both are libraries, so Cupel ships a small runner for one and runs it against a pinned module version, as it ships the Clojure test runner (ADR-109) and the Python harnesses. Porcupine answers the check's question (linearizability) for the register and key-value histories most agent-written systems produce, runs in about a second with `go run`, and needs only Go, which CI already has.

**The excerpt.** Porcupine's verbose result gives, per partition, the longest linearizable prefixes; the operations it could not place next are those not in the prefix that real time allows next (invoked before the first of them returns). The prefix's last operations, the model's state after it, and those stuck operations show the anomaly as a person reads it ("`read 1` cannot follow state `2`").

**Support.** The history is language-neutral, so the checker serves every language; registering it with `Langs::Any` would leave its availability unable to say which language's `[lang.<id>]` table lacks the command (availability sees no language). Registering one instance per language of the toolchain table (every language has a row there, design §8.3.1) keeps `unconfigured` per language.

## Decision

1. **`histories` v1** (`cupel_core::Histories`, design §8.3.2 row, schema, providable by producer plugins):

   ```text
   Histories {
     tool: ProducerId,              // tool.porcupine
     command: String,               // [lang.<id>] history_command
     seed: u64,                     // CUPEL_SEED the command was given
     run: RootRun,                  // the command's run from the repository root: ran | failed (non-zero exit, output's tail)
     histories: [History {
       name,                        // the file, relative to the history directory
       model: Option<String>,       // declared on the first line
       operations: u64,             // invocations that completed (ok) or whose outcome is unknown
       status: linearizable | violated | unknown | invalid,
       detail: Option<String>,      // invalid: why, with its line; unknown: the time limit
       violations: [HistoryViolation {
         key: Option<Value>,        // a kv history's key; null for one object
         anomaly: String,           // "non-linearizable" (an Elle producer would name its anomaly type)
         states: [Value],           // the model's states after the longest linearizable prefix
         linearized: [HistoryOp],   // that prefix's last three operations, in order
         stuck: [HistoryOp],        // the operations real time allows next, none of which the model allows
       }],
     }],
   }
   HistoryOp { process: Value, f, key: Option<Value>, value: Value, outcome: ok | info, line }   // line: its invocation's
   ```

   Its conformance inventory `cupel-fixtures/conformance/histories.toml` lists `linearizable`, `stale-read`, `indeterminate`, `unfinished`, `failed-op`, `compare-and-set`, `per-key`, `unknown-model`, `unmatched-completion`. An execution fact's inventory is checked by its producers' real-tool tests over their fixture repository, whose `inventory.toml` maps each construct to a history (`conformance::EXECUTION`, a `locate` arm by history name); the language harness checks only syntactic facts.

2. **Recording.** `[lang.<id>] history_command` (a string) names the command. The producer runs it with `sh -c` from the repository root, with `CUPEL_HISTORY_DIR` set to `.cupel/cache/linearizability/<lang>/` (emptied first) and `CUPEL_SEED` to the run's seed, and reads every `*.jsonl` file under that directory, by relative path. A non-zero exit is a `failed` run (its output's tail), and the histories it wrote are still checked. A history file: the first non-blank line `{"model": "<name>"}`; then one event per line, `{"process": <scalar>, "type": "invoke"|"ok"|"fail"|"info", "f": "<function>", "value": <any>, "key"?: <any>}`; a completion pairs with its process's open invocation (same `f`); an invocation never completed is unknown, as `info`. A completion without an invocation, an invocation while the process has one open, a different `f`, a missing `process`, or another `type` makes the history `invalid` with its line.

3. **`tool.porcupine`** (`cupel-adapt/src/linearizability/`): one producer per language of the toolchain table, cost `deep`, needs nothing; `unconfigured` (`lang.<id>.history_command`) without the command, else ready when `go` is on `PATH` (else missing, with the `[toolchains]` hint). Models: `register` (`read`, `write`, `cas` with value `[expected, new]`) and `kv` (the same per key, partitioned by key in the order keys first appear, each operation naming its key); any other model or function is `invalid`. Values compare as canonical JSON (keys sorted). A failed operation is dropped; an operation of unknown outcome returns after the last line (so it may take effect anywhere after its call, or never); a read of unknown outcome observed nothing and is dropped. Cupel's runner (`porcupine.go`, written with its `go.mod` pinning `github.com/anishathalye/porcupine v1.3.1` and `go.sum` into `.cupel/cache/linearizability/porcupine/`) is run once per run with `go run .` (`GOTOOLCHAIN=local`, `GOFLAGS=-mod=readonly`); it reads every partition of every history as JSON on standard input, checks each with `CheckOperationsVerbose` against a register model whose state is canonical JSON text (`null` at first), and prints each result (`Ok`, `Illegal`, `Unknown`) with the longest partial linearization (the lexicographically least among the longest, so it repeats) and the state after it. Its input and output stay there as `request.json` and `response.json`. Time limit per partition: 30 s below `exhaustive`, 300 s at `exhaustive`. A history with an `Illegal` partition is `violated` (one violation per such partition), else one with an `Unknown` partition is `unknown`, else `linearizable`. A runner that fails (Porcupine cannot be downloaded, the module does not build) is an error of the producer, not a verdict.

4. **`conc.linearizability`** (`cupel-checks/src/conc_linearizability.rs`): tier 5, cost `deep`, needs `Histories`, no settings, stability `experimental`. **`consistency.violation`** (`fail`, confidence 1, `fix_bug`) per violation; subject `<history>` or `<history>|<canonical key>`; no location (a history is not source); title `` history `<name>` (<model>)[ key `<key>`] is <anomaly>: `<f> <value>` by process <p> (line <n>)[ and N more operation(s)] cannot follow state `<s>` `` (`cannot be ordered` without states); evidence `{history, model, key, anomaly, tool, states, linearized, stuck, command, seed}`; the action says `CUPEL_SEED=<seed> <command>` records the history again; repro `cupel check --only conc.linearizability --seed <seed>`. Metrics, summed across languages: `histories`, `operations`, `violations`, `unknown`, `invalid`; score `{value: violations, unit: "violations", n: histories}`; status `fail` with any violation. Notes: a failed command, a command that wrote no history, each history not decided (unknown) or not checked (invalid), with its detail.

## Consequences

- `conc.linearizability` is `unconfigured` for every language until `[lang.<id>] history_command` is set, then supported wherever Go is installed; there is no `unsupported` cell, since any project can write a history. The matrix golden is the Go fixture (`repos/conc.linearizability/go/`), whose command copies committed histories, one per conformance construct; a real harness records them as it runs.
- A history is only as good as the harness's real-time order: events must be written in the order they happened (a single writer, or per-process buffers merged by a monotonic clock before writing). A harness that randomizes its schedule reads `CUPEL_SEED` so the repro repeats it.
- Porcupine's search is NP-hard; a large history can run out of its time limit, which is a note, never a pass or a finding.
- The first run downloads Porcupine from the Go module proxy (`GOPROXY`); offline, the producer errors with Go's message.
- Not done, with the reason: **Elle** (Clojure, `elle.list-append` / `elle.rw-register`) checks transactional histories, a different history shape (a transaction of micro-operations per event) and question (isolation levels, anomaly types G0–G2) from single-object linearizability; it needs its own models in the history format, and since the registry selects one producer per fact and language, a way to send each history to the checker of its model (a pin key, or model-based routing); its JVM start and Clojars resolution add tens of seconds per run. Knossos (Jepsen's linearizability checker) is superseded by Porcupine's speed and is not wrapped. Models beyond register and kv (a set, a queue, a counter) need Go code in the runner each.
