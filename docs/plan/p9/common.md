# P9 agent briefs: the rules every brief shares

Every P9 brief in this directory is read together with this file. A brief names its branch, its scope, the ADR number it may use, and the modules it must leave alone; this file holds everything else.

## Where you work

- You are in a git worktree of Cupel on the branch the brief names. Never run git commands against another worktree or `main`. Never push. Never `cd` out of the worktree for git operations (`git -C` if you must look elsewhere, read-only).
- `make dev-setup` installs the pinned tool environment (ADR-139) and every `make` target exports it; run it first. `cargo` run by hand does not see the tools unless you `eval "$(make dev-env)"`.
- Disk: a fresh worktree's build is 15 to 30 GB and a coverage run doubles it; the machine has filled up twice. Run `make reclaim` (the coverage build, the incremental caches, the suite cache) after every `make dogfood` and before you report; never build into a second `target` (`CARGO_TARGET_DIR`) under your scratchpad; delete each external clone's build outputs (`target`, `node_modules`, `.venv`, `build`) as soon as its report is written. `scripts/heavy.sh` refuses to start a heavy command below 40 GB free and tells you what to run.
- `timeout` does not exist on macOS. In zsh, quote strings like `'===='`. Run anything that takes more than a few minutes with `nohup ... > LOG 2>&1 &` under your scratchpad directory and poll the log.
- `make dogfood` waits on a machine-wide lock (`scripts/heavy.sh`) so one heavy command runs at a time across every checkout. Any other heavy command you run, above all an external project's test suite or a `cupel survey` or `cupel check` over one, goes through the same script, one invocation per command (`scripts/heavy.sh <command>`, never one around a loop over repositories), so the lock is released between repositories. `make check` does not take the lock. The integrator's merge chains have priority (`CUPEL_HEAVY_PRIORITY=first`); never set it yourself. Wait on the lock; never poll the load average and never start a second dogfood of your own.

## How you work

- Read `docs/CUPEL_IMPLEMENTATION_PLAN.md` §1–§2 (rules, ADR format §2.4, §4 cross-phase checklist) and the sub-step heading the brief names, and `docs/CUPEL_TECHNICAL_DESIGN.md` for the parts you touch, before writing code.
- Rigorous TDD: write the failing test, run it and see it fail for the right reason, then the smallest change that passes, then refactor. Integration tests drive the real binary and real tools; no mocks of what can run for real.
- Pristine repository: every file starts with the two `ABOUTME:` comment lines; comments are evergreen (what the code is, never what it was or how it improved); no markdown outside `README.md` and `docs/`; no `todo!()`, no commented-out code, no stray files.
- The fact model is strict: no language name in `cupel-checks`, no check name in `cupel-lang`, `cupel-adapt`, or `cupel-mutate`. `describe` must reflect every new verb, flag, check, finding kind, fact, or producer.
- An ADR (`docs/adr/CUPEL_DECISION_NNN.md`, §2.4 format, the number the brief reserves) is written before the code that depends on it. Amending or superseding an older ADR changes that ADR's status line and links the new one.
- Goldens: regenerate with `UPDATE_GOLDENS=1 INSTA_UPDATE=always make test`, then `make readme`, `make fmt`, and review the diff. The diff must be additive (new checks, fields, facts, findings). A changed pre-existing value is a regression unless the notes justify it line by line. Never commit a golden you have not read.
- `README.md` describes only what exists. Its generated sections come from `make readme`; hand-written prose changes only when behaviour changed.

## Before you report

1. `make check` green (fmt, clippy `-D warnings`, the whole test suite, docs, and the clean-tree check).
2. `make parity` green when you touched the CLI, the renderers, the envelope, or the daemon.
3. `make dogfood` (the changed gate, at the depth `Makefile` sets) exits 0 on your branch. Fix what it finds in your own code; a finding in code you did not touch is reported, not suppressed.
4. The plan: set the sub-step's status line (`**Status:** complete (YYYY-MM-DD, <sha>; [ADR-NNN](adr/CUPEL_DECISION_NNN.md))`) and add bullets to `#### P9 Notes` named `**P9.N — design**`, `**P9.N — tests**` (count before and after, each new test named with what it proves), `**P9.N — dogfood**` (the exact command and result), `**P9.N — open items**`, and `**P9.N — for the integrator**` (every file changed, which are yours and which are shared, and what another branch must do if it touches the same snapshot or schema). Keep every other line of the plan as it is.
5. Commit on your branch, often, with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>` as the last line of every message. Leave nothing uncommitted.

## The report

Under 60 lines: branch tip sha; the ADR's decision in two sentences; the files changed; the test count before and after; the dogfood command and result; what was verified for real and what only by inspection; open items. No prose about process.
