// ABOUTME: The toolchain table (design §8.7): per language, launchers and tool producers in default order
// ABOUTME: with detection rules and command templates, selection for a root, and its `describe` form.
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::LazyLock;

use cupel_core::{
    CommandTemplate, LangId, LauncherDescription, ProducerId, ToolDescription, ToolId,
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

/// A tool producer: a runner, coverage, or bench producer.
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
    /// The first launcher and runner whose rules hold at `root`, then the first coverage and
    /// bench producers whose rules hold and that have a command for that runner.
    pub fn select<'a>(&'a self, root: &Root) -> Selection<'a> {
        let dirs = root.dirs();
        let launcher = self
            .launchers
            .iter()
            .find(|launcher| detected(&launcher.detect, &dirs));
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
        }
    }
}

impl Selection<'_> {
    /// The default commands, with the launcher's values and `<packages>` filled in.
    pub fn commands(&self) -> Commands {
        let Some(runner) = self.runner else {
            return Commands::default();
        };
        let fill = |template: &str| {
            let mut command = template.replace("<packages>", &self.packages);
            for (name, value) in self.launcher.iter().flat_map(|launcher| &launcher.values) {
                command = command.replace(&format!("<{name}>"), value);
            }
            command
        };
        let companion =
            |tool: Option<&Tool>| tool.and_then(|tool| tool.command_for(&runner.id)).map(fill);
        Commands {
            test: runner.command.as_deref().map(fill),
            coverage: companion(self.coverage),
            bench: companion(self.bench),
        }
    }
}
