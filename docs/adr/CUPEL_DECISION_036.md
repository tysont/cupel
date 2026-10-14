# CUPEL_DECISION_036: The toolchain table is TOML data in `cupel-adapt` with declarative detection rules

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.9

## Context

Design §8.7 puts a **toolchain table** in `cupel-adapt`, keyed by `LangId`: per language, the runners Cupel knows with their detection rules (lockfile, config file, dependency name), the coverage producers and the report formats they emit, the bench producers, and the property, fuzz, and race tools. The row is the registry's default producer order for that language; `init` reads it for default commands, `doctor` for what to verify, and `describe language` renders it. Plan P1.9 creates the crate with the table before any tool producer exists (P3). Until now the same knowledge lived as Rust code in `cupel-cli/src/detect/*.rs` (ADR-021 §3). The design does not say how the table is written, how a detection rule is expressed, how a coverage command depends on the runner it wraps (`c8 … mocha` versus `c8 … node --test`), or how the package-manager prefixes (`npx`, `pnpm`, `uv run python -m`) are chosen.

## Options

**Form.** (a) A `const`/`LazyLock` Rust structure: typed and checked by the compiler, but every rule is a closure or an enum literal, and adding a tool means editing Rust. (b) TOML embedded with `include_str!` and parsed once, like each language's `tables.toml`: declarative, reviewable as data, and adding a tool is one table entry; malformed data fails a test instead of the compiler.

**Rules.** (a) Free-form predicates per tool in code. (b) A small closed rule vocabulary evaluated by `cupel-adapt` against a project root on disk: a file exists, a file contains a string, a manifest mentions a package name, a JSON manifest has a value at a path (optionally with a prefix), and `all` of several rules. This covers every rule ADR-021 §3 uses with no language-specific code.

**Runner-dependent commands.** (a) One coverage command per producer, with a `{test}` placeholder for the runner's command: `npx c8 … npx mocha` is not what anyone runs. (b) A coverage (or bench) producer lists its command per runner it works with; a producer with a single `command` works with every runner.

**Package managers.** (a) Bake the prefix into each command per manager (three copies of every Node command). (b) **Launchers**: an ordered list per row, each with detection rules and named values (`x = "pnpm"`, `run = "pnpm run"`, `m = "uv run python -m"`), substituted into `<name>` placeholders when `init` fills the commands. `<…>` is distinct from the `{out}`/`{name}`/`{id}` placeholders that stay in `cupel.toml` for the producers to fill at run time (design §6.7).

## Decision

- `crates/cupel-adapt/src/toolchain.toml` holds the table, embedded with `include_str!` and parsed once (`toolchain::table()`); `toolchain::for_lang(&LangId) -> Option<&Row>` finds a row. A row names its languages (`javascript` and `typescript` share one), its launchers, runners, coverage producers (with report format), and bench producers, each in default order, and its property, fuzz, and race tool names. `[probes]` gives the presence probe for programs whose `--version` does not work (`go version`).
- A tool entry has a producer ID (`tool.<name>`), `detect` rules (any one selects it; none means it is the fallback), and either `command` or `commands` keyed by runner ID. Rules: `{ file }`, `{ contains, in }`, `{ mentions, in = [globs] }` (a whole package name, as ADR-021 defines mentioning), `{ json, path, prefix?, members? }`, `{ all = [...] }`. `file`, `contains`, and `json` read the root directory; `mentions` and `json` with `members = true` also read the root's workspace members.
- Selection for a root (`Row::select`): the first launcher whose rules match, the first such runner, then the first coverage and bench producers whose rules match and that have a command for that runner. `Selection::commands` substitutes the launcher's values and `<packages>` (from `ProjectInfo`, ADR-037).
- The table's content is exactly ADR-021 §3's rules and commands, so `init` output is unchanged; coverage formats are `lcov` everywhere except Go's `coverprofile`, because every default command asks for LCOV.
- `cupel-adapt` depends on `cupel-core` only (design §8.2); its crate docs state the rule.

## Consequences

- P3 implements each `tool.<name>` producer in `cupel-adapt` and registers them in row order, so the registry's default order is the table's order (design §8.3.3).
- Adding a tool is one table entry plus, in P3, its producer; adding a language's row is data only.
- A rule the vocabulary cannot express needs a new rule kind in `cupel-adapt`, reviewed once, rather than code per tool.
- The design's table lists Istanbul JSON for vitest and jest; the defaults request LCOV instead, so the row says `lcov`. A future producer that reads Istanbul JSON adds an entry, not a format change.
