# CUPEL_DECISION_102: `static.analyze`: the `AnalyzerReport` payload, one analyzers producer per language, and `analyze.<tool>.<rule>` kinds

**Date:** 2026-10-16 · **Status:** accepted; mypy is not handed a module that has a stub beside it per [138](CUPEL_DECISION_138.md) §9 · **Phase:** P6.2

## Context

Design §5.1: `static.analyze` "runs the ecosystem's type checker at strictest config, linters, CodeQL/Semgrep if configured; normalizes to findings", kinds `analyze.<tool>.<rule>`, wrap, tier 1. The catalog (§8.3.2) has an `AnalyzerReport` row, "SARIF-normalized results" from "linter/type-checker tool", but `FactKind::AnalyzerReport` has no payload type (`schema::fact` returns `None` for it, and producer plugins cannot provide it). The toolchain table (§8.7, ADR-036) lists runners, coverage, bench, property, fuzz, and race tools, not analyzers. Open: the payload's shape; how several analyzers of one language (Python's mypy and ruff, Go's `go vet` and staticcheck) feed one check when the registry selects one producer per (kind, language) (ADR-076); which configuration "strictest" means; what a missing or unconfigured analyzer does to the support matrix; and how a manifest declares kinds whose tool and rule are only known when the tool reports them, when every finding kind must be in the manifest (ADR-012, ADR-043) and the check may not know any tool's name (design §8.3.1).

## Options

**Payload.** (a) SARIF itself (`runs[].results[]`): the standard, but large, optional almost everywhere, and every consumer re-derives the same few fields. (b) The SARIF subset a check needs, per tool: rule ID, level (`error`, `warning`, `note`), message, and one physical location, with the tool's run status beside its results. (b); a SARIF file a tool writes can be read into it by a parser later.

**Several analyzers per language.** (a) One producer per tool: the registry would pick the first ready one and drop the rest, unless the check needed one fact kind per tool. (b) One producer per language that runs every analyzer its toolchain row lists and records each one's status in the fact. (b): the row is the default order (ADR-036), a pin is not needed to run two tools, and the check reads one fact.

**A missing analyzer.** (a) The producer is `unavailable` when any listed analyzer is missing: one missing linter would hide the type checker's results. (b) Ready when at least one is installed, with each missing one recorded (and its install command) in the fact, which the check turns into a note; `unavailable` with the first one's install hint when none is. (b).

**Strictest configuration.** Type checkers get their strict mode on the command line (`mypy --strict`, `tsc --strict`), which raises what the project configured. Linters keep the project's configuration: there is no tool-neutral "strictest" for a linter (`clippy::pedantic`, ruff's `ALL`) that is not mostly style noise, and a project's own allowances (Cupel's `[workspace.lints]`) are deliberate. ESLint needs a flat configuration file; without one it refuses to run, so it is `unconfigured` there rather than run with an invented rule set.

**Kinds.** (a) One declared kind (`analyze.diagnostic`) with tool and rule in the evidence: no core change, but loses the design's per-rule kinds that `describe finding`, suppressions by kind, and SARIF `ruleId` read. (b) Declare every tool's kinds in the check: the check would name tools. (c) A kind *template* `analyze.<tool>.<rule>` in the manifest, where a `<name>` segment stands for one segment of the reported kind and a final one for the rest: the check stays tool-neutral and every reported kind is still declared. (c), matched by one function the matrix harness and `describe finding` share.

## Decision

1. **`analyzer-report` v1** (`cupel_core::AnalyzerReport`), providable by producer plugins like every kind with a payload (ADR-088 §8):

```text
AnalyzerReport { analyzers: [AnalyzerRun {
  tool: ToolId,                                   // "mypy", "go-vet"
  status: "ran" | "missing" | "unconfigured" | "failed",
  detail: Option<String>,                         // missing: how to install; unconfigured: what to add; failed: the output's tail
  results: [AnalyzerResult { rule: String, level: "error" | "warning" | "note", message: String, location: Location }],
}] }
```

   Results are those in files of the producer's scope, relative to the repository root, in tool order, without exact duplicates (a lint reported for a library and its test build).

2. **`tool.analyzers`**, one producer per language whose toolchain row lists analyzers, needs `ProjectInfo`, cost `standard`, runs each listed analyzer in each project root of the language (the repository root when none is detected). Ready when at least one analyzer's program is installed (on `PATH`, or in the root's `node_modules/.bin` for Node tools), else `unavailable` naming the first with its install command. The toolchain table gains `[[rows.analyzers]]` (`id`, `install`, `detect`), shown by `describe language`:

| Language | Analyzers (order) | Invocation | Level |
|---|---|---|---|
| `rust` | `tool.clippy` | `cargo clippy --all-targets --message-format=json` | rustc's (`warning`/`error`), rule without `clippy::` |
| `python` | `tool.mypy`, `tool.ruff` | `mypy --strict --output json --cache-dir <.cupel/cache> <files>`; `ruff check --output-format json --no-cache <files>` | mypy's; ruff `warning` |
| `javascript` | `tool.eslint` | `eslint --format json <files>`, only with a flat config in the root | 2 `error`, 1 `warning` |
| `typescript` | `tool.tsc`, `tool.eslint` | `tsc --noEmit --strict --pretty false` with `-p` when the root has `tsconfig.json`, else the files | `error` |
| `go` | `tool.go-vet`, `tool.staticcheck` | `go vet -json <packages>`; `staticcheck -f json <packages>` | vet `warning`; staticcheck's |

   An analyzer whose output cannot be read, or that exits with a code its tool uses for a crash, is `failed` with the tail of its output; the others still run. Nothing is written in the tree beyond what the tool itself keeps there by convention (cargo's `target/`).

3. **Kind templates.** `FindingKindSpec::matches(kind)`: equal, or the spec contains `<…>` segments, each standing for one segment of `kind`, the last for one or more. `describe finding <kind>` and the matrix harness resolve a reported kind through it; plugin manifests still declare exact kinds.

4. **`static.analyze`**: tier 1, cost `standard`, needs `AnalyzerReport`, no settings, stability `experimental`, one declared kind `analyze.<tool>.<rule>` (action `fix_bug`). Each result is a finding of kind `analyze.<tool>.<rule>`, where the rule's characters outside `[A-Za-z0-9_.-]` become `-`, dashes and dots are trimmed from its ends, and empty dot segments are dropped (`unnamed` when nothing is left); severity `fail` for `error`, `warn` for `warning`, `info` for `note`; confidence 1; located at the result; evidence `{tool, rule, level, message}`; ID `FindingId::for_subject(check, kind, "<file>|<message>|<n>")` (`n` counts identical results in the file), stable across line moves; repro `cupel check --only static.analyze --paths <file>`. Each analyzer that did not run is a note (`<tool> <status>: <detail>`). Score `{value: results, unit: "diagnostics", n: analyzers that ran}`, metrics `errors` and `diagnostics` summed across languages, none when no analyzer ran; status `fail` when any result is above a note (as the scheduler grades every check with a `warn` or `fail` finding).

## Consequences

- One fact serves every analyzer of every language; a new analyzer is a row entry and a parser, and a CodeQL or Semgrep producer would emit the same records (as a separate analyzer in a row, or a plugin).
- Results depend on tool versions; the goldens are made with the versions CI pins, and the cost class keeps the check out of `quick` runs (design §5.8: "except `analyze` (if slow)").
- Diagnostics in files outside the scope are dropped by the producer, so `--changed` reports only changed files, although the tools analyze the whole project to type-check them.
- ESLint runs only where a project configured it; TypeScript without a `tsconfig.json` is checked file by file with the compiler's defaults plus `--strict`.
- A tool's own configuration (a `mypy.ini`, `ruff.toml`, `[lints]`) is honoured, so the same code can report differently in two projects; that is the project's choice to make.
