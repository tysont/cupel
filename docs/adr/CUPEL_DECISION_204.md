# CUPEL_DECISION_204: The store records the files each run's parses used, and the sweep counts the runs that used any

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, item 5 · Details [201](CUPEL_DECISION_201.md)

## Context
ADR-201 has the store's sweep delete `parsed_facts` rows "whose file hash is absent from the files of the last 20 runs". No run recorded the files it read: a `parsed_facts` row's `run_id` is the run that derived it, and a file whose values the store already holds, or a warm daemon holds in memory, is read without its row changing. Nor was there a sweep: ADR-188 records a run's values in one transaction after its run record and prunes nothing but another build's rows. Every verb records a run, including `explain`, `baseline` and `suppress`, which parse nothing.

## Options
**Which files a run used.** (a) Rewrite each row's `run_id` to the latest run that read it: a `WITHOUT ROWID` row is rewritten with its payload, about 18 MB on Cupel's repository per run, and the latest user alone cannot say which runs were the last 20. (b) A table of (run, file key) pairs, written with the run's values: a few hundred small rows per run, 20 runs of them kept.

**Which runs count.** (a) The last 20 rows of `runs`: twenty `explain` runs, which read no file, would drop every file's values. (b) The last 20 runs that used any parsed file.

**What "file hash" is.** (a) The content hash alone. (b) The row's file key (ADR-188 §2: path, content hash, language, grammar), which is what rows are stored and read by.

## Decision
1. Migration 8 adds `parsed_uses (run_id, file_key)`, keyed by both. `FactShelf::used(key)` is called by every parse that has a shelf, whether the file's values come from the store, from a warm daemon's memory, or from its tree; the CLI's shelf collects the keys until the run is recorded.
2. `Store::record_parsed_facts(build, run, facts, used)` writes the run's values and its uses in one transaction, then sweeps: it deletes the uses of every run but the last 20 (`PARSED_RUNS_KEPT`) that have any, by run ID (a ULID, so by start), and every `parsed_facts` row whose file key no remaining use names. A run that derived nothing but used files records its uses and sweeps; a run that used none (no parse, or a verb that reads no file) records nothing and does not count.
3. The file is the row's file key: a file moved to another path, or parsed with another grammar, is another key.
4. A store from before migration 8 has no uses, so its first sweep keeps only the files that run used.

## Consequences
The table holds the values of files one of the last 20 parsing runs used, about the current tree's plus recent history; a file reverted to a version no recent run used is parsed again. `parsed_uses` holds at most 20 runs' file keys (about 14 000 rows on Cupel's repository). The store's tests cover a file dropped after 20 runs that did not use it and kept through the 20th, and the CLI shelf's test covers a run that derives nothing.
