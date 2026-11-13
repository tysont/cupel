# CUPEL_DECISION_188: The fact cache keeps each file's derived values, by build, in a table of its own

**Date:** 2026-11-12 · **Status:** accepted; amended by [201](CUPEL_DECISION_201.md) (owner rulings, 2026-11-13) · **Phase:** v0.2.7, item 9 · Details [181](CUPEL_DECISION_181.md)

## Context
ADR-181 stores "per-file facts the static checks derive from a parse" by content hash, language, grammar version, and Cupel's version, and has the no-daemon CLI read them. It does not say which values those are, what "Cupel's version" is for a build that changes without a version bump, where in the store they go (the `facts` table of ADR-045 §6 is keyed by a §8.3.6 `CacheKey` naming one fact kind), how they are written, or how large the store gets.

A profile of `CUPEL_NO_DAEMON=1 cupel check --depth quick` on Cupel's repository (666 files parsed) settled what to keep. Of the CPU the syntactic producers spent, parsing trees was about 13%; running queries over the trees to extract each file's share of a fact was about 80%, and resolving test helpers across files, which reran every time (the warm daemon too), took another 0.4 s on one thread. Trees cannot be serialized, and keeping them would save the smaller part.

## Options
**What to keep.** (a) Trees: not serializable, and not where the time goes. (b) Whole facts by a key over every file's hash: any edit misses the whole fact. (c) Every value `Parsed::derived` makes from one file, by an extractor name: what the warm daemon keeps in memory (ADR-085 §3), so a cached run does what a warm daemon does.

**Cupel's version.** (a) The crate version: a development build changes without one, and would read values an older build of the same version derived. (b) The build, as the daemon already compares it (ADR-085 §4): the version and the executable's size and modification time.

**Where.** (a) `facts` through `put_fact`: one row and one write transaction per value, and a fact kind for values that are not facts (a file's unresolved references, its first syntax error). (b) A table of its own, read once per file and written once per run.

**Encoding.** (a) The values' JSON: about 150 MB for Cupel's repository, most of it `norm-tree` and `references`. (b) Their JSON deflated at the fastest level: about one ninth of that.

## Decision
1. **What.** Every value `Parsed::derived` makes is kept: each generic producer's per-file part (named by its fact kind: `symbols`, `decisions`, `norm-tree`, `mutation-sites`, `test-frames`, `api-surface`, `roots`), a file's imports (`imports`) and references before resolution (`references`), a test file's asserting helpers (`test-helpers`, now derived per file, so the warm daemon keeps them too), and the position of its first syntax error (`first-error`). What spans files (merging parts, module resolution, helpers shared by a directory) runs every time over the kept parts.
2. **Key.** A file's key is BLAKE3 of its path (extractors read it: every symbol key names its file), its content hash, its language, and its grammar's name, version, and ABI. Each row also carries the build: `<version> (binary <bytes>-<mtime>)`. A rebuilt binary misses every key; one whose executable cannot be read keeps nothing.
3. **Lazy trees.** A file whose values the store holds is read and hashed but not parsed; its tree is parsed only if something asks for a value the store lacks. A value that does not read back is derived again.
4. **Where.** Migration 7 adds `parsed_facts (build, file_key, name, run_id, payload)`, keyed by the first three. `Store::parsed_facts` reads a file's values in one query; `Store::record_parsed_facts` writes a run's in one transaction and deletes every row of another build, so the store holds one build's values: every content version that build has seen. `facts` and `put_fact` are left for whole facts by `CacheKey`, which nothing caches yet.
5. **Encoding.** Each payload is the value's JSON deflated at level 1 (`miniz_oxide`, pure Rust). The values hold no floats, so JSON reads back exactly.
6. **When.** Values derived during a run are held in memory and recorded after the run record, by every run that records one (not a dry run, not `describe`); a dry run reads a store that exists and creates none. A failed write is the usual line, `cupel: could not record the run in the fact store: …`, and changes nothing else.
7. **Who.** Each process gives its session's parse cache the store of the first repository it runs in: the cold CLI, and the daemon, which then reads the store only for files its memory lacks, so a restarted daemon starts warm.
8. **Seeing it.** `--verbose` prints `cupel: parsed facts: N files read from the fact store, M trees parsed`.

## Consequences
Output is the same with and without kept values: `fact_goldens` holds every fact golden of every language for facts read from a shelf with no tree parsed, and `fact_cache` compares a quick check that reads every file's values with one that parses every file, on each language fixture. The first run in a repository, and the first after a rebuild, also serializes, deflates, and writes the values (on Cupel's repository 18 MB, about a tenth of a second more); every later run of the same build reads them. The store's size is bounded by one build's values; nothing prunes the content versions of files that have since changed, as nothing prunes runs (ADR-045 §9).
