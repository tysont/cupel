// ABOUTME: Root of cupel-adapt, the tool layer: the per-language toolchain table (design §8.7) and,
// ABOUTME: from P3, the tool producers it names (runners, coverage, bench).
//! Cupel's tool layer.
//!
//! **Dependency rule:** this crate depends on `cupel-core` and nothing else from Cupel. It never
//! depends on `cupel-lang` (languages) or `cupel-checks` (checks), and neither depends on it
//! (design §8.2); only `cupel-cli` joins a language's `ProjectInfo` with this crate's table.
//!
//! # The toolchain table
//!
//! `src/toolchain.toml` is the table: one row per language (or languages sharing tools), each
//! listing launchers, runners, coverage producers, and bench producers in default order, plus
//! property, fuzz, and race tool names. [`toolchain::for_lang`] finds a row,
//! [`toolchain::Row::select`] picks a root's tools by their detection rules, and
//! [`toolchain::Selection::commands`] fills the default commands `init` writes (ADR-036).
//!
//! ```toml
//! [[rows]]
//! langs = ["python"]
//! property = ["hypothesis"]
//!
//! [[rows.launchers]]                 # first match wins; values fill `<name>` placeholders
//! tool = "uv"
//! detect = [{ file = "uv.lock" }, { contains = "[tool.uv", in = "pyproject.toml" }]
//! values = { m = "uv run python -m" }
//!
//! [[rows.runners]]                   # first match wins; no `detect` means always
//! id = "tool.pytest"
//! detect = [{ mentions = "pytest", in = ["pyproject.toml", "requirements*.txt"] }]
//! command = "<m> pytest"
//!
//! [[rows.coverage]]                  # `commands` per runner, or one `command` for every runner
//! id = "tool.pytest-cov"
//! format = "lcov"
//! commands = { "tool.pytest" = "<m> pytest --cov --cov-report=lcov:{out}" }
//! ```
//!
//! Detection rules (any one in a list selects the tool): `{ file }` exists in the root
//! directory; `{ contains, in }` a root file contains the text; `{ mentions, in }` a file in the
//! root or a member, matched by glob, names the package (not as part of a longer name);
//! `{ json, path, prefix?, members? }` a JSON file has a value at `path` (a string starting
//! with `prefix`, when given), in the root or also in members; `{ all = [...] }` every rule.
//! `<packages>` is filled from `ProjectRoot.packages`; `{out}`, `{name}`, and `{id}` stay for
//! the producers to fill at run time (design §6.7).

mod rules;
pub mod toolchain;

pub use rules::Rule;
