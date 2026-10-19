# CUPEL_DECISION_126: The agent guide, `required_version` outside `doctor`, and README sections from `describe`

**Date:** 2026-10-18 · **Status:** accepted · **Phase:** P8.3

## Context

Design §6.5 lists `cupel describe --agent  # ~60 lines markdown for CLAUDE.md / AGENTS.md`, and §12's P8 row asks for "docs generated from `describe --all`". Until this step `describe --agent` parsed, was described with `pending_step: P8.3`, and exited 2 with `cli.not-implemented`; it was the last user of that error. Design §6.5 says `doctor` "checks binary version vs `required_version`", and §10 says "`doctor` enforces". ADR-022 §1 made `doctor` the only verb that reads `required_version` and kept the actual version out of the finding (it is the envelope's `cupel.version`), so an old binary still ran `check` against a repository that requires a newer one and graded it with whatever its checks happened to be. The README (plan §2.7) generated its exit-code, check, check-settings, support-matrix, and MCP-tool tables (ADR-073, ADR-121), but its global-flag table was hand-written, it had no verb list, and its Agents section described the loop in prose that could drift from the binary. The questions:

1. What the guide says, where its text lives, and how much of it comes from the registries.
2. Where the guide goes in the envelope, and what `--format text` prints.
3. Which verbs refuse to run outside `required_version`, at which point, with which finding and exit code, and whether anything lifts the refusal.
4. Which README sections are generated, and how the guide appears in the README.

## Options

**Guide text: a template file, a Rust module, or entirely generated.** A Markdown template under `docs/` or `crates/` with placeholders keeps prose editable as Markdown but needs a template engine or a placeholder parser and splits the guide between two files. Generating everything from the registries produces lists, not instructions: the loop, the gate's meaning, and what to do on exit 3 are prose. A Rust module holding the prose in one `write!` with named arguments for the rendered parts keeps it in one place, readable in order, with the compiler checking every placeholder.

**Guide in the envelope: a new `description` subject, a top-level field, or text only.** `description` is already tagged by `subject`; a new variant `agent` with one `markdown` field adds no top-level field to the run schema and keeps `describe`'s shape. Text only would make `--format json` the one `describe` that carries no description.

**`--format text`: the usual status line and run block around the guide, or the guide alone.** The guide is meant to be redirected into `AGENTS.md`; a status line and run block would have to be cut by hand every time. The exit code still says how the run ended.

**Where the version is enforced.** In `Config::load` (core), every caller would see it, but core does not depend on `semver`, and `doctor` and `init` must run with configuration that fails it. At the CLI's dispatch, after configuration loads and before the verb runs, the decision is per verb and the configuration is already parsed. A `--no-version-check` escape would let an agent with the wrong binary grade anyway, which is exactly what the key exists to stop; the requirement is changed in `cupel.toml` (reviewed) or the finding suppressed (with a reason, in a committed file).

**Which verbs refuse.** All but `doctor` would also stop `describe` (an agent's first command, which must describe the binary it has), `init` (which rewrites the file holding the requirement), `suppress` (the documented way to accept the mismatch for a while), and `serve` (whose served requests each load configuration as a cold run). Everything else reads or grades the repository or its store: `check`, `survey`, `tests`, `explain`, `baseline`, `oracle`.

**Malformed requirements outside `doctor`.** `doctor` exits 3 for any `fail` finding (ADR-022 §5). For the other verbs a value in `cupel.toml` that does not parse is a usage error (design §6.6, exit 2), as every other configuration mistake is.

**README guide: inline Markdown or a fenced block.** Inlined, the guide's `# Cupel` and `##` headings would become README headings and break its outline; fenced as ```` ```markdown ````, it shows exactly what to paste.

## Decision

1. **The guide is `crates/cupel-cli/src/agent.rs`.** `agent::guide(checks)` returns about sixty lines of Markdown: what Cupel is (the registry's summary plus one sentence), the loop (`cupel check --changed --format json`, `next_actions[0]`, the finding's `repro`, rerun until exit 0; `cupel explain`, `cupel tests --impacted --changed`, `describe <check-id>`, `describe finding`, `--dry-run`, `--describe`), the exit-code table, what to do on exits 2, 3, and 4, the gate's semantics with two example rules, the depth flags, a table of the checks each depth adds, how to read the envelope (`status`, `exit_code`, `findings`, `repro`, `next_actions`, `summary.gate`, `truncated`), `cupel serve --mcp` as the alternative (ADR-121) with its tools, and the verbs. Rendered, not typed: the summary and verbs and exit codes (`registry::cli()`), `--depth`'s values, default, and summary and `--budget`'s (`check`'s registered arguments), the checks per depth (the catalog's manifests by `cost`, plugins included as `describe matrix` includes them), the `status` values (the run schema's `Status`), and the MCP tool names (`mcp::tools()`).
2. **JSON: `description: {"subject": "agent", "markdown": …}`** (`cupel_core::AgentDescription`). **Text: the Markdown alone**, ending in one newline, so `cupel describe --agent --format text > AGENTS.md` needs no editing. `--agent` loses its `pending_step`, gains an example, and its MCP `describe` tool takes `agent`.
3. **`CupelError::NotImplemented`, `Pending`, `pending_description`, and the `cli.not-implemented` finding are removed**, having no user left; tests that used `describe --agent` as their failing command use `describe schema nope` (`cli.usage`).
4. **`required_version` refuses at dispatch.** After configuration loads and the `--describe` early return, `app::execute` calls `doctor::refusal` for every verb but `describe`, `doctor`, `init`, `suppress`, and `serve`. The refusal is doctor's own finding (`check: doctor`, same ID): `doctor.version-unsatisfied`, exit 3 (`environment`), `install_tool` "Install a cupel matching `<req>`, or change required_version."; or `doctor.version-invalid`, exit 2, `configure`. Dry runs are refused too. The repository's suppressions are applied to the refusal as to any `doctor` run: a suppressed `doctor.version-unsatisfied` refuses nothing. No `--no-version-check` flag.
5. **The version finding carries `{required, actual}`** (superseding ADR-022 §1's `{required}`), so an agent reading one finding sees both. Golden redaction masks a finding's `evidence.actual` when it equals the envelope's `cupel.version`, so goldens still survive releases.
6. **README sections generated from `describe`:** `verbs` (every verb's summary, at the top of Usage), `global-flags` (flag with its values or value name, default, variable, summary; pipes escaped), and `agent-guide` (the guide, fenced, in the section renamed "For agents"), beside the existing five. `crates/cupel-cli/tests/readme.rs` renders all eight from the binary and fails when any differs; `make readme` rewrites them. The `listed_tools` helper moves into the CLI tests' `support` module, shared by `readme.rs` and `agent.rs`.
7. **Tests pin the guide:** `tests/agent.rs` snapshots the text, asserts 60 ± 10 lines, that JSON carries the same Markdown, that every `--flag` in its code spans is a registered flag of some command or the CLI, that every check ID or `<check>.<metric>` in its code spans is registered (the metric in that check's manifest), and that every exit-code row and MCP tool (from a real `tools/list`) appears.

## Consequences

- An agent harness can paste `describe --agent` output and keep it current by rerunning it; the README shows the same text and fails `make check` when it drifts.
- A repository can pin the Cupel it was graded with: an old binary refuses rather than grading with a different check set. `cupel doctor` and `cupel describe` still work on it, so an agent learns why and what to install.
- The guide's prose names flags and checks by hand; the registered-name tests catch a rename or removal, but not a newly added flag the prose should mention.
- Adding a check changes the guide's depth table, its snapshot, and the README's `agent-guide` section, as it already changes the check table; `make readme` and `INSTA_UPDATE=always` resolve both.
- The global-flag table now says what the registry says; richer prose about a flag belongs in the registry's summary or in the README's text around the table.
