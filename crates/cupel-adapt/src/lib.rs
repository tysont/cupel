// ABOUTME: Root of cupel-adapt, the tool layer: the per-language toolchain table (design §8.7) and the
// ABOUTME: tool producers it names (runners, coverage, bench), each submitting itself (ADR-074).
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

//!
//! # Adding a tool producer
//!
//! A tool producer is one module here that submits itself (ADR-074):
//!
//! ```text
//! inventory::submit!(crate::ToolProducer {
//!     id: "tool.pytest",
//!     register: |registry| registry.register::<TestResults, _>(Pytest),
//! });
//! ```
//!
//! Beside the module file, it adds its `mod <name>;` line here and edits nothing else in this
//! file; [`register`], which the CLI and the matrix harness already call, picks it up.

pub mod report;
mod rules;
pub mod toolchain;

use cupel_core::ProducerRegistry;

pub use rules::Rule;

/// A tool producer, submitted by its own module with `inventory::submit!`; [`register`] applies
/// every submission.
pub struct ToolProducer {
    /// The producer's ID, such as `tool.pytest`; submissions are applied in ID order.
    pub id: &'static str,
    /// Adds the producer to a registry.
    pub register: fn(&mut ProducerRegistry),
}

inventory::collect!(ToolProducer);

/// Adds every tool producer to `registry`, in ID order. Which producer of a fact a run uses is
/// the toolchain table's and the configuration's choice, not this order.
///
/// # Panics
///
/// When two submissions share an ID: tool producer IDs are unique.
pub fn register(registry: &mut ProducerRegistry) {
    let mut tools: Vec<&ToolProducer> = inventory::iter::<ToolProducer>.into_iter().collect();
    tools.sort_by_key(|tool| tool.id);
    if let Some(pair) = tools.windows(2).find(|pair| pair[0].id == pair[1].id) {
        panic!(
            "tool producer IDs are unique: `{}` is submitted twice",
            pair[0].id
        );
    }
    for tool in tools {
        (tool.register)(registry);
    }
}
