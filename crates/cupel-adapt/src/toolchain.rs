// ABOUTME: The toolchain table (design §8.7): per language, launchers and tool producers in default order
// ABOUTME: with detection rules and command templates, selection for a root, and its `describe` form.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use cupel_core::{
    CommandTemplate, Install, LangId, LauncherDescription, ProducerId, ToolDescription, ToolId,
    ToolchainDescription,
};
use serde::Deserialize;

use crate::rules::{Dirs, Rule};

const TABLE: &str = include_str!("toolchain.toml");

static TOOLCHAIN: LazyLock<Toolchain> = LazyLock::new(|| {
    toml::from_str(TABLE).unwrap_or_else(|error| panic!("toolchain.toml: {error}"))
});

/// The whole table.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Toolchain {
    /// Presence probes for programs whose `--version` does not work.
    #[serde(default)]
    probes: BTreeMap<String, String>,
    /// What to do by hand for programs Cupel does not install (ADR-081).
    #[serde(default)]
    toolchains: BTreeMap<String, String>,
    rows: Vec<Row>,
}

/// The tools Cupel knows for one or more languages, each list in default order.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub langs: Vec<LangId>,
    #[serde(default)]
    pub launchers: Vec<Launcher>,
    pub runners: Vec<Tool>,
    #[serde(default)]
    pub coverage: Vec<Tool>,
    #[serde(default)]
    pub bench: Vec<Tool>,
    /// Type checkers and linters, run in this order (ADR-102).
    #[serde(default)]
    pub analyzers: Vec<Tool>,
    #[serde(default)]
    pub property: Vec<ToolId>,
    #[serde(default)]
    pub fuzz: Vec<ToolId>,
    #[serde(default)]
    pub race: Vec<ToolId>,
}

/// A prefix commands are launched through, such as a package manager.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Launcher {
    pub tool: ToolId,
    /// Any one selects it; none makes it the fallback.
    #[serde(default)]
    pub detect: Vec<Rule>,
    /// What each `<name>` placeholder becomes when this launcher is chosen.
    #[serde(default)]
    pub values: BTreeMap<String, String>,
}

/// A tool producer: a runner, coverage, or bench producer, or an analyzer.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub id: ProducerId,
    /// The report format, for coverage producers.
    #[serde(default)]
    pub format: Option<String>,
    /// Any one selects it; none makes it always eligible.
    #[serde(default)]
    pub detect: Vec<Rule>,
    /// The command for every runner.
    #[serde(default)]
    pub command: Option<String>,
    /// The command per runner it works with.
    #[serde(default)]
    pub commands: BTreeMap<ProducerId, String>,
    /// The command that installs it, filled with the launcher's values (ADR-081).
    #[serde(default)]
    pub install: Option<String>,
}

/// A project root on disk: its directory, its workspace members' directories, and the package
/// patterns its commands cover (`<packages>`).
pub struct Root {
    dir: PathBuf,
    members: Vec<PathBuf>,
    packages: Vec<String>,
}

/// The tools chosen for one root.
pub struct Selection<'a> {
    pub launcher: Option<&'a Launcher>,
    pub runner: Option<&'a Tool>,
    pub coverage: Option<&'a Tool>,
    pub bench: Option<&'a Tool>,
    packages: String,
}

/// Default commands for a root; each is absent when no tool applies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Commands {
    pub test: Option<String>,
    pub coverage: Option<String>,
    pub bench: Option<String>,
}

/// Every row, in table order.
pub fn rows() -> &'static [Row] {
    &TOOLCHAIN.rows
}

/// The row for `lang`, if the table has one.
pub fn for_lang(lang: &LangId) -> Option<&'static Row> {
    rows().iter().find(|row| row.langs.contains(lang))
}

/// Where the producer `id` stands in the table's default order: its row, then its place among
/// the row's runners, coverage, and bench producers; after every listed producer when the table
/// does not list it.
pub fn position(id: &str) -> (usize, usize) {
    rows()
        .iter()
        .enumerate()
        .find_map(|(row_index, row)| {
            row.tools()
                .position(|tool| tool.id.as_str() == id)
                .map(|index| (row_index, index))
        })
        .unwrap_or((usize::MAX, usize::MAX))
}

/// Whether the producer `id`'s detection rules hold at the repository root `root` (a tool with
/// no rules always holds; a tool the table does not list never does).
pub fn detects(id: &ProducerId, root: &Path) -> bool {
    let dirs = Dirs { root, members: &[] };
    rows()
        .iter()
        .flat_map(Row::tools)
        .find(|tool| tool.id == *id)
        .is_some_and(|tool| detected(&tool.detect, &dirs))
}

/// The table's command for the runner `id` in `root`, with the launcher detected there and the
/// root's packages filled in; `None` when the table lists no command for it.
pub fn runner_command(id: &ProducerId, root: &Root) -> Option<String> {
    let row = rows()
        .iter()
        .find(|row| row.runners.iter().any(|tool| tool.id == *id))?;
    let mut selection = row.select(root);
    selection.runner = row.runners.iter().find(|tool| tool.id == *id);
    selection.commands().test
}

/// The runners the coverage producer `id` works with, by the table's `commands`; empty when it
/// has one `command` for every runner, or the table does not list it.
pub fn companions(id: &ProducerId) -> Vec<&'static ProducerId> {
    rows()
        .iter()
        .flat_map(|row| &row.coverage)
        .find(|tool| tool.id == *id)
        .map(|tool| tool.commands.keys().collect())
        .unwrap_or_default()
}

/// How to install `program`, which the producer `id` runs, for the repository at `root`
/// (ADR-081).
/// The row's `install` command with the launcher detected at `root` filled in, else the
/// `[toolchains]` hint for `program`; `None` when the table knows neither.
pub fn install(id: &ProducerId, program: &str, root: &Path) -> Option<Install> {
    let listed = rows().iter().find_map(|row| {
        let template = row.tools().find(|tool| tool.id == *id)?.install.as_ref()?;
        let dirs = Dirs { root, members: &[] };
        Some(fill(template, row.launcher(&dirs), ""))
    });
    match listed {
        Some(command) => Some(Install::Command(command)),
        None => TOOLCHAIN
            .toolchains
            .get(program)
            .map(|hint| Install::Hint(hint.clone())),
    }
}

/// A command that exits 0 when `program` is installed: `[probes]`, else `<program> --version`.
pub fn probe(program: &str) -> Vec<String> {
    TOOLCHAIN.probes.get(program).map_or_else(
        || vec![program.to_owned(), "--version".to_owned()],
        |probe| probe.split_whitespace().map(str::to_owned).collect(),
    )
}

impl Root {
    pub fn new(dir: PathBuf, members: Vec<PathBuf>, packages: Vec<String>) -> Self {
        Self {
            dir,
            members,
            packages,
        }
    }

    fn dirs(&self) -> Dirs<'_> {
        Dirs {
            root: &self.dir,
            members: &self.members,
        }
    }
}

fn detected(rules: &[Rule], dirs: &Dirs<'_>) -> bool {
    rules.is_empty() || rules.iter().any(|rule| rule.holds(dirs))
}

impl Row {
    /// Every tool of the row: runners, then coverage and bench producers, then analyzers.
    fn tools(&self) -> impl Iterator<Item = &Tool> {
        self.runners
            .iter()
            .chain(&self.coverage)
            .chain(&self.bench)
            .chain(&self.analyzers)
    }

    /// The first launcher whose rules hold in `dirs`.
    fn launcher(&self, dirs: &Dirs<'_>) -> Option<&Launcher> {
        self.launchers
            .iter()
            .find(|launcher| detected(&launcher.detect, dirs))
    }

    /// The first launcher and runner whose rules hold at `root`, then the first coverage and
    /// bench producers whose rules hold and that have a command for that runner.
    pub fn select<'a>(&'a self, root: &Root) -> Selection<'a> {
        let dirs = root.dirs();
        let launcher = self.launcher(&dirs);
        let runner = self
            .runners
            .iter()
            .find(|tool| detected(&tool.detect, &dirs));
        let companion = |tools: &'a [Tool]| -> Option<&'a Tool> {
            let runner = runner?;
            tools.iter().find(|tool| {
                tool.command_for(&runner.id).is_some() && detected(&tool.detect, &dirs)
            })
        };
        Selection {
            launcher,
            runner,
            coverage: companion(&self.coverage),
            bench: companion(&self.bench),
            packages: root.packages.join(" "),
        }
    }

    /// The row as `describe language` shows it.
    pub fn describe(&self) -> ToolchainDescription {
        ToolchainDescription {
            launchers: self
                .launchers
                .iter()
                .map(|launcher| LauncherDescription {
                    tool: launcher.tool.clone(),
                    detect: launcher.detect.iter().map(Rule::describe).collect(),
                    values: launcher.values.clone(),
                })
                .collect(),
            runners: self.runners.iter().map(Tool::describe).collect(),
            coverage: self.coverage.iter().map(Tool::describe).collect(),
            bench: self.bench.iter().map(Tool::describe).collect(),
            analyzers: self.analyzers.iter().map(Tool::describe).collect(),
            property: self.property.clone(),
            fuzz: self.fuzz.clone(),
            race: self.race.clone(),
        }
    }
}

impl Tool {
    /// The command this tool uses alongside `runner`, if it works with it.
    pub fn command_for(&self, runner: &ProducerId) -> Option<&str> {
        self.commands
            .get(runner)
            .or(self.command.as_ref())
            .map(String::as_str)
    }

    fn describe(&self) -> ToolDescription {
        ToolDescription {
            producer: self.id.clone(),
            format: self.format.clone(),
            detect: self.detect.iter().map(Rule::describe).collect(),
            commands: self
                .command
                .iter()
                .map(|command| CommandTemplate {
                    runner: None,
                    command: command.clone(),
                })
                .chain(
                    self.commands
                        .iter()
                        .map(|(runner, command)| CommandTemplate {
                            runner: Some(runner.clone()),
                            command: command.clone(),
                        }),
                )
                .collect(),
            install: self.install.clone(),
        }
    }
}

impl Selection<'_> {
    /// The default commands, with the launcher's values and `<packages>` filled in.
    pub fn commands(&self) -> Commands {
        let Some(runner) = self.runner else {
            return Commands::default();
        };
        let fill = |template: &str| fill(template, self.launcher, &self.packages);
        let companion =
            |tool: Option<&Tool>| tool.and_then(|tool| tool.command_for(&runner.id)).map(fill);
        Commands {
            test: runner.command.as_deref().map(fill),
            coverage: companion(self.coverage),
            bench: companion(self.bench),
        }
    }
}

/// `template` with `<packages>` and each of the launcher's `<name>` placeholders filled in.
fn fill(template: &str, launcher: Option<&Launcher>, packages: &str) -> String {
    let mut command = template.replace("<packages>", packages);
    for (name, value) in launcher.iter().flat_map(|launcher| &launcher.values) {
        command = command.replace(&format!("<{name}>"), value);
    }
    command
}
