# CUPEL_DECISION_236: The memory sampler lists into a file, never holds its lock across a listing, and skips a listing that runs past 5 s

**Date:** 2026-11-15 · **Status:** accepted; the fixture test's bound amended on 2026-11-16: the looping mutants' own runs (`run_ms`) add up to under 25 s, since the check's whole duration also counts the coverage, reference, and build runs a slow CI runner stretches (23.5 s on macos-latest) · **Phase:** v0.3.0, release blocker found by gate b52 · Amends [148](CUPEL_DECISION_148.md) §3

## Context

ADR-148 gave every mutant run a memory budget, measured from `ps` samples that all of a process's wait loops share. Each wait loop polls every 10 ms. It checks whether the child exited, then whether its cancellation fired (a mutant's timeout is a deadline on that cancellation), then whether the run holds more than its budget. That last check read the shared sample. A reader that found the sample older than 200 ms ran `ps` itself, holding the sample's lock, and read `ps`'s output from a pipe to its end.

On 2026-11-15, gate b52 hung for five hours in `cupel-mutate`'s `tests/stack.rs`.

- **Stacks:** a stack sample of the test binary showed one thread inside `memory::list`, blocked in `read` on the `ps` pipe. Every other wait loop, eight tests' mutant, start-up and reference runs, waited for the sample's lock.
- **Processes:** four Python mutants, each an infinite loop (`i -= 1`, `i += 0`), had been running since the gate reached the test. Zombies of finished runs sat beside them.
- **The pipe:** `lsof` showed one of the mutants holding both ends of the binary's `ps` pipe.
- **Confirmation:** killing the four Pythons let all eight tests pass at once.

Two faults combined:

1. **The trigger.** On macOS, std creates a pipe with `pipe()` and only then sets close-on-exec on each end. A child that another thread spawns between the two inherits both ends, and keeps them through `exec`. A mutant spawned in that window held the `ps` pipe open for its whole life, so the listing never saw the end of its output.
2. **The failure mode.** The listing waited while holding the lock that every wait loop takes. No wait loop could reach its cancellation check, so no mutant's timeout fired, including the timeout of the mutant holding the pipe. One leaked descriptor became a deadlock that only an outside kill could break.

The owner accepted the diagnosis and the first fix on 2026-11-15, and ruled, verbatim: "Narrow the sampler's lock. The memory-budget sampler must not hold the lock shared by the mutant wait loops across the ps call or any other blocking I/O; take it only to read or write shared state. Give the ps invocation its own timeout (5 s); on timeout, skip that sample, record a sampler.timeout event, and continue — the budget fails open rather than stalling timeouts. Add a test with a ps stand-in that blocks: all mutant timeouts must still fire on schedule. Record in the ADR that the pipe fix removes the trigger and the lock change removes the failure mode."

## Decision

1. **The listing goes to a file.** `ps` writes to an unlinked temporary file, and the sampler reads the file once `ps` has exited. A process that inherits the descriptor can no longer delay the end of the listing, because nothing waits for an end of file. *This removes the trigger.*
2. **The lock covers only shared state.** The lock guards four fields: the latest sample, whether a listing is under way, whether the sampler is stalled (item 4), and the count of timed-out listings. The lock is taken only to read or write them.
   - A wait loop that finds the sample fresh uses it.
   - Otherwise the loop marks a listing under way, starts it on its own thread, and returns no sample at once. That poll skips the memory check, and the next poll finds the new sample.
   - A loop that finds a listing already under way returns at once as well. It starts its own listing only when the one under way started before its child's spawn and so cannot serve it, and only while the sampler is not stalled (item 4). Otherwise a run would wait for two listings before its first sample, and gate b54's dogfood saw a 0.74 s nextest run with a 1-byte budget end unmeasured. Of two listings under way, the one that started later is kept.

   No wait loop ever waits on `ps`, so a listing that hangs can delay no timeout. *This removes the failure mode.*
3. **A listing has its own 5 s timeout.** A listing still running after 5 s is killed and skipped, and counted. The scheduler reports each count as one `{"ev":"sampler.timeout","timeout_ms":5000}` event, just before the `check.end` of the check whose producers ran. Until a listing ends in time, the memory budget goes unmeasured: it fails open.
4. **A kill does not list while the sampler is stalled.** A timed-out listing marks the sampler stalled, and the next listing that ends in time clears the mark. While the sampler is stalled, the kill of a timed-out or cancelled run signals only the child's own process group, without listing for groups its descendants moved to. Otherwise a kill would wait up to 5 s per listing, at least twice, after its timeout had fired.
5. **Which instant counts.** A sample counts as taken when its listing started, for ADR-148 §3's rule that a reader never reads a sample from before its child's spawn returned. Its 200 ms age is counted from when the listing ended, so a listing slower than 200 ms still serves the readers that follow it.

## Consequences

- With `ps` replaced by a program that never exits, every listing is skipped at 5 s and the looping mutants still end at their cap. The fixture test is `a ps that never answers` in `crates/cupel-fixtures/tests/mutant_timeout.rs`; with a 10 s cap the check ends within 20 s and the events stream carries `sampler.timeout`. The unit tests in `memory.rs` cover:
  - a listing whose output a background process holds
  - a listing that outlasts its timeout
  - a reading taken while a listing is under way
- A run's first poll after its spawn never has a sample, because the sample must start after the spawn returned. The memory budget is first measured one listing after the spawn, about 0.2 s at a load of 130 on the development machine. That is the same lag as before this ADR, when the first poll listed inline, but it is no longer spent blocked inside a wait loop. A run that ends within one listing ends unmeasured, as it did before.
- While the sampler is stalled, a run can exceed its memory budget unnoticed. Its kill can also miss a descendant in another process group, which then outlives the run. Both are what failing open costs, and the `sampler.timeout` events make that window visible.
- The pipe leak itself is std's behaviour on macOS and remains. Other pipes Cupel reads to their end (a run's own stdout and stderr) are read on reader threads, not under a shared lock. A leaked copy of one delays only that run's output, and only until the process holding it ends or is killed at its own timeout.
