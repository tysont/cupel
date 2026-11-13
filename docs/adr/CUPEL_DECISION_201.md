# CUPEL_DECISION_201: The sweep prunes parsed facts no recent run has used

**Date:** 2026-11-13 · **Status:** accepted · **Phase:** v0.2.8, the owner's rulings after v0.2.7 · Amends [188](CUPEL_DECISION_188.md)

## Context
ADR-188 stores parsed facts in the `parsed_facts` table keyed by file content hash and never removes a row, so the table grows with every edit.

## Options
(a) Never prune. (b) Prune by age. (c) Prune rows whose hash none of the last 20 runs read (the owner's ruling).

## Decision
The store's sweep deletes `parsed_facts` rows whose file hash is absent from the files of the last 20 runs.

## Consequences
The table holds about the current tree's facts plus recent history; a file reverted to a version older than 20 runs is parsed again. v0.2.8 implements it; the plan's batch v0.2.8 Notes record the implementation and its tests.
