// ABOUTME: `tool.footprint` (ADR-103): one producer per language that reads its projects' lockfiles and manifests
// ABOUTME: offline into a `Footprint` with licenses, OSV advisories, configured artifact sizes, and the range's base side.
//! The footprint producer.
//!
//! `tool.footprint` is one [`Producer`] of [`Footprint`] per language with an [`Ecosystem`]. In
//! every project root of the language `ProjectInfo` names, the ecosystem chooses the lockfiles
//! and manifests to read ([`Ecosystem::discover`]) and parses each into dependencies
//! ([`Ecosystem::parse`]); licenses come from the lockfile itself or from what the ecosystem
//! keeps on disk, which discovery reads into the [`Context`]. Advisories (`[lang.<id>] advisories`, [`osv`]) and artifacts
//! (`[lang.<id>] artifacts`, [`artifacts`]) are read only when configured. Under a range the
//! same files are read as they were at the base commit (`Focus::base`) through git. Nothing
//! touches the network or writes in the tree.

mod artifacts;
mod cargo;
mod gomod;
mod npm;
mod osv;
mod python;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use cupel_core::git::Git;
use cupel_core::syntax::ProjectInfo;
use cupel_core::{
    Availability, Cancellation, CostClass, Ctx, Dependency, Error, EventSink, FactKind, Footprint,
    FootprintBase, LangId, Langs, ProduceCtx, Producer, ProducerId, Result, Scope,
};

const ID: &str = "tool.footprint";

/// The `[lang.<id>]` key naming a directory of OSV advisories.
pub const ADVISORIES_KEY: &str = "advisories";

/// The `[lang.<id>]` key listing artifact globs.
pub const ARTIFACTS_KEY: &str = "artifacts";

/// What one ecosystem's projects are read with.
pub struct Ecosystem {
    /// The languages it serves.
    pub langs: &'static [&'static str],
    /// Its name in OSV advisories (`crates.io`, `PyPI`, `npm`, `Go`).
    pub osv: &'static str,
    /// What to read in the project root `dir` (relative to the repository root, absolute at
    /// `root`): the files, relative to the repository root, and what parsing them needs.
    pub discover: fn(&Root<'_>) -> Project,
    /// The dependencies `text`, the contents of `file`, lists.
    pub parse: fn(&str, &str, &Context) -> Vec<Dependency>,
    /// A dependency name as advisories compare it.
    pub normalize: fn(&str) -> String,
}

/// A project root being read.
pub struct Root<'a> {
    /// Relative to the repository root; `.` for the root itself.
    pub dir: &'a str,
    /// Absolute.
    pub path: PathBuf,
    pub cancel: &'a Cancellation,
}

impl Root<'_> {
    /// `name` in this root, relative to the repository root.
    pub fn file(&self, name: &str) -> String {
        if self.dir == "." {
            name.to_owned()
        } else {
            format!("{}/{name}", self.dir)
        }
    }
}

/// What one project root has to read.
#[derive(Debug, Default)]
pub struct Project {
    /// The files to parse, relative to the repository root, in order.
    pub manifests: Vec<String>,
    pub context: Context,
}

/// What parsing needs beyond a file's text, read at head.
#[derive(Debug, Default, Clone)]
pub struct Context {
    /// The project's own packages, which are not dependencies.
    pub members: BTreeSet<String>,
    /// The names the project declares itself, normalized.
    pub direct: BTreeSet<String>,
    /// Licenses read from disk, by `name@version` or by name; `None` when the project's
    /// licenses are not read (or come from its lockfile, which parsing reads).
    pub licenses: Option<BTreeMap<String, String>>,
    /// Whether the lockfile itself records licenses.
    pub licensed_lockfile: bool,
}

impl Context {
    /// Whether licenses were read for this project.
    pub fn licensed(&self) -> bool {
        self.licenses.is_some() || self.licensed_lockfile
    }

    /// The license read from disk for `name` at `version`, by exact version, else by name.
    pub fn license(&self, name: &str, version: Option<&str>) -> Option<String> {
        let licenses = self.licenses.as_ref()?;
        version
            .and_then(|version| licenses.get(&format!("{name}@{version}")))
            .or_else(|| licenses.get(name))
            .cloned()
    }
}

static ECOSYSTEMS: [&Ecosystem; 4] = [&cargo::CARGO, &python::PYTHON, &npm::NPM, &gomod::GO];

/// Reads a language's dependencies offline (ADR-103).
pub struct FootprintReader(&'static Ecosystem, LangId);

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| {
        for ecosystem in ECOSYSTEMS {
            for lang in ecosystem.langs {
                registry.register::<Footprint, _>(FootprintReader(ecosystem, LangId::from(*lang)));
            }
        }
    },
});

impl Producer<Footprint> for FootprintReader {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.1.clone()])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<Footprint> {
        let ecosystem = self.0;
        let mut footprint = Footprint::default();
        let mut projects = Vec::new();
        for dir in project_dirs(ctx) {
            ctx.cancel.checkpoint()?;
            let root = Root {
                dir: &dir,
                path: ctx.ctx.root.join(&dir),
                cancel: &ctx.cancel,
            };
            let project = (ecosystem.discover)(&root);
            ctx.cancel.checkpoint()?;
            for file in &project.manifests {
                if let Ok(text) = std::fs::read_to_string(ctx.ctx.root.join(file)) {
                    let listed = (ecosystem.parse)(file, &text, &project.context);
                    footprint.licenses |= project.context.licensed() && !listed.is_empty();
                    footprint.dependencies.extend(listed);
                }
            }
            footprint
                .manifests
                .extend(project.manifests.iter().cloned());
            projects.push(project);
        }
        footprint.vulnerabilities = osv::configured(ctx, ecosystem, &footprint.dependencies)?;
        footprint.artifacts = artifacts::configured(ctx)?;
        footprint.base = base(ctx, ecosystem, &projects);
        Ok(footprint)
    }
}

/// The project roots of the language being produced: none when `ProjectInfo` names none.
fn project_dirs(ctx: &ProduceCtx) -> Vec<String> {
    ctx.upstream
        .get::<ProjectInfo>()
        .map(|info| info.roots.iter().map(|root| root.path.clone()).collect())
        .unwrap_or_default()
}

/// The dependencies the same files listed at the range's base commit; `None` outside a range
/// or a git work tree.
fn base(ctx: &ProduceCtx, ecosystem: &Ecosystem, projects: &[Project]) -> Option<FootprintBase> {
    let commit = ctx.focus.base.as_ref()?;
    let git = Git::open(&ctx.ctx.root)?;
    let mut dependencies = Vec::new();
    for project in projects {
        for file in &project.manifests {
            let Ok(bytes) = git.show(commit, file) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            dependencies.extend((ecosystem.parse)(file, &text, &project.context));
        }
    }
    Some(FootprintBase {
        commit: commit.clone(),
        dependencies,
    })
}

/// The line (1-based) of the first line of `text` at or after `from` (0-based) that `matches`;
/// with that index, so the next search starts after it.
pub(crate) fn line_after(text: &str, from: usize, matches: impl Fn(&str) -> bool) -> (u32, usize) {
    text.lines()
        .enumerate()
        .skip(from)
        .find(|(_, line)| matches(line.trim()))
        .map_or((1, from), |(index, _)| {
            (u32::try_from(index + 1).unwrap_or(u32::MAX), index + 1)
        })
}

/// An error reading a file a producer was configured to read.
pub(crate) fn unreadable(path: &Path, problem: &str) -> Error {
    Error::Process {
        program: ID.to_owned(),
        message: format!("cannot read {}: {problem}", path.display()),
    }
}
