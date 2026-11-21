# CUPEL_DECISION_241: npm publishing refuses a run with no terminal, since its sign-in and its publish each wait on one

**Date:** 2026-11-21 · **Status:** accepted · **Phase:** v0.3.1 follow-up · Amends [238](CUPEL_DECISION_238.md) §4 (what an npm publish needs)

## Context

ADR-238 §4 has `make publish-npm` sign in with `npm login --auth-type=web` and then `npm publish`, with the owner confirming in the browser. It says a publish "needs the owner at the browser".

Publishing v0.3.1 showed that it needs a terminal as well:

- Twice, the step ran without a terminal on stdin: once started by the agent, and once through the agent session's `!` prompt, which moves a command to the background after 120 s. Each time, npm printed the sign-in URL and polled until the session expired (npm logged `web login not supported, trying couch`). It then fell back to a `Username:` prompt with nothing to answer it. Nothing was published, and no session was created.
- Run from the owner's own terminal, it worked. npm asked for two confirmations: one to sign in, and a second at `npm publish` ("Authenticate your account at: …"). Each was offered with "Press ENTER to open in the browser".

## Options

1. Refuse a non-dry-run npm publish when stdin is not a terminal, with a message that names the command to run in the owner's own terminal.
2. Leave it. A run without a terminal then waits about five minutes for a sign-in nobody can finish, then fails at the username prompt.

## Decision

1. `publish_npm` in `scripts/ship.sh` checks `[ -t 0 ]` before any other check, unless the run is a dry run. Without a terminal it exits 1 with: "npm's browser sign-in and its publish each wait on a terminal for your confirmation; run make publish-npm V=X.Y.Z in a terminal of your own". Nothing is checked, signed in to, or published. `make publish` runs npm first, so it is refused the same way. A dry run needs no terminal.

## Consequences

- An agent cannot publish to npm. It hands the step to the owner, who runs it in a terminal of their own.
- PyPI and the Homebrew tap need no terminal. They still run from an agent's session.
- The Makefile's release comment says `make publish` runs in a terminal of the owner's own.
- Tests: `cupel-cli/tests/ship.rs` `publishing_to_npm_needs_a_terminal`, observed failing first, when the refusal came from the missing tag instead.
