# CUPEL_DECISION_181: Parsed facts persist in the store, keyed by content hash, and the CLI reads them

**Date:** 2026-11-11 · **Status:** accepted; detailed by [188](CUPEL_DECISION_188.md) · **Phase:** v0.2.7, the owner's rulings on the Molten field trial · Amends [045](CUPEL_DECISION_045.md)

## Context
`Store::put_fact` had no caller, so every command-line run re-parsed every file: about 31 s of static checks on Molten against 10 s through a warm daemon.

## Options
(a) Keep parsing in the CLI and point users at the daemon. (b) Persist parsed facts in the store, keyed by file content hash, and read them on the no-daemon path (the owner's ruling).

## Decision
Per-file facts the static checks derive from a parse are stored keyed by the file's content hash, with the language, the grammar version, and Cupel's version in the key. A CLI run without a daemon reads a file's facts from the store when the key matches and parses only on a miss. Target: a cold-CLI run's parse on Molten within 1.5× the warm daemon's.

## Consequences
The store grows with the facts of every content version seen; the existing store bounds apply. A grammar or Cupel upgrade misses every key, which is correct. v0.2.7 implements it; the plan's batch v0.2.7 Notes record the implementation and its tests.
