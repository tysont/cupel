// ABOUTME: Project roots (`ProjectInfo`, ADR-037): the walk that finds an ecosystem's manifests, the
// ABOUTME: nesting rule that makes workspace members, and the producer each language registers with it.
//!
//! An [`Ecosystem`] is a language's build files: the manifests that mark its roots, the
//! directories it generates, and how a root is described. Each language defines its ecosystem
//! in `langs/<dir>/project.rs` and registers [`ProjectRoots`] with it. The walk visits
//! directories in sorted order, skipping hidden ones and every known ecosystem's generated
//! directories, so a language's roots do not depend on which other languages exist. A
//! directory with one of the ecosystem's manifests is a root, or a member of the root above it
//! when there is one (ADR-021 §2).
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Availability, CostClass, Ctx, EventSink, Exclude, FactKind, LangId, Langs, ProduceCtx,
    Producer, ProducerId, Result, Scope,
};

use crate::langs::{ecma, go, python, rust};
use crate::language::Language;
use crate::producer::producer_id;

/// One build ecosystem: the files that mark its roots, the directories it generates, and how a
/// root of it is described.
pub struct Ecosystem {
    pub manifests: &'static [&'static str],
    pub skip_dirs: &'static [&'static str],
    pub describe: fn(&Candidate) -> ProjectRoot,
}

/// Maven and Gradle roots. Java is not registered until P6; `init` reports these (ADR-038).
const JAVA: Ecosystem = Ecosystem {
    manifests: &[
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    ],
    skip_dirs: &["target", "build", ".gradle"],
    describe: |candidate| candidate.root("java", Vec::new()),
};

/// `deps.edn` roots. Clojure is not registered until P6; `init` reports these (ADR-038).
const CLOJURE: Ecosystem = Ecosystem {
    manifests: &["deps.edn"],
    skip_dirs: &["target", ".cpcache"],
    describe: |candidate| candidate.root("clojure", Vec::new()),
};

/// Every ecosystem, whose generated directories every walk skips.
const ECOSYSTEMS: [&Ecosystem; 6] = [
    &rust::project::ECOSYSTEM,
    &go::project::ECOSYSTEM,
    &ecma::project::ECOSYSTEM,
    &python::project::ECOSYSTEM,
    &JAVA,
    &CLOJURE,
];

/// A directory holding an ecosystem's manifest, with the directories below it that hold that
/// ecosystem's manifests too (workspace members).
pub struct Candidate {
    ecosystem: &'static Ecosystem,
    /// Relative to the repository root, `/`-separated; `.` for the root itself.
    pub path: String,
    pub dir: PathBuf,
    pub members: Vec<PathBuf>,
}

impl Candidate {
    /// The root directory, then each member directory.
    pub fn dirs(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.dir.as_path()).chain(self.members.iter().map(PathBuf::as_path))
    }

    /// The text of `relative` in the root directory, if it can be read.
    pub fn read(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.dir.join(relative)).ok()
    }

    /// The `ProjectRoot` for this candidate in `lang`: its manifests, then its members'
    /// manifests, its members, `packages`, and the ecosystem's generated directories.
    pub fn root(&self, lang: &str, packages: Vec<String>) -> ProjectRoot {
        let build_files = self
            .dirs()
            .flat_map(|dir| {
                self.ecosystem
                    .manifests
                    .iter()
                    .filter(move |manifest| dir.join(manifest).is_file())
                    .map(move |manifest| relative(&dir.join(manifest), &self.dir))
            })
            .collect();
        ProjectRoot {
            path: self.path.clone(),
            lang: LangId::from(lang),
            build_files,
            members: self
                .members
                .iter()
                .map(|member| relative(member, &self.dir))
                .collect(),
            packages,
            skip_dirs: self
                .ecosystem
                .skip_dirs
                .iter()
                .map(|dir| (*dir).to_owned())
                .collect(),
        }
    }
}

/// Every root of `ecosystem` under `root` outside `exclude`, ordered by path, then language.
pub fn roots(root: &Path, ecosystem: &'static Ecosystem, exclude: &Exclude) -> Vec<ProjectRoot> {
    let mut candidates = Vec::new();
    walk(root, root, ecosystem, exclude, &mut candidates);
    let mut roots: Vec<ProjectRoot> = candidates
        .iter()
        .map(|candidate| (ecosystem.describe)(candidate))
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    roots
}

/// Roots of languages that are recognized but not registered (Java, Clojure) outside
/// `exclude`, ordered by path, then language.
pub fn unregistered_roots(root: &Path, exclude: &Exclude) -> Vec<ProjectRoot> {
    let mut roots: Vec<ProjectRoot> = [&JAVA, &CLOJURE]
        .into_iter()
        .flat_map(|ecosystem| roots(root, ecosystem, exclude))
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    roots
}

/// Visits `dir` and the directories below it in sorted order, never entering one `exclude`
/// excludes. A directory with one of the ecosystem's manifests becomes a root, or a member of
/// the root above it when there is one.
fn walk(
    root: &Path,
    dir: &Path,
    ecosystem: &'static Ecosystem,
    exclude: &Exclude,
    candidates: &mut Vec<Candidate>,
) {
    if ecosystem
        .manifests
        .iter()
        .any(|manifest| dir.join(manifest).is_file())
    {
        match candidates
            .iter_mut()
            .find(|candidate| dir.starts_with(&candidate.dir))
        {
            Some(candidate) => candidate.members.push(dir.to_path_buf()),
            None => candidates.push(Candidate {
                ecosystem,
                path: relative(dir, root),
                dir: dir.to_path_buf(),
                members: Vec::new(),
            }),
        }
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut subdirs: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            !name.starts_with('.') && !generated(&name)
        })
        .map(|entry| entry.path())
        .filter(|subdir| !exclude.excludes(&relative(subdir, root)))
        .collect();
    subdirs.sort();
    for subdir in subdirs {
        walk(root, &subdir, ecosystem, exclude, candidates);
    }
}

/// Whether some ecosystem generates directories named `name`.
fn generated(name: &str) -> bool {
    ECOSYSTEMS
        .iter()
        .any(|ecosystem| ecosystem.skip_dirs.contains(&name))
}

/// `path` relative to `base`, `/`-separated; `.` when they are the same.
fn relative(path: &Path, base: &Path) -> String {
    let parts: Vec<String> = path
        .strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

/// The `ProjectInfo` producer: a language's roots of its ecosystem. It reads the repository
/// root, not the scope, because project layout is repository-wide.
pub struct ProjectRoots {
    language: Arc<dyn Language>,
    ecosystem: &'static Ecosystem,
}

impl ProjectRoots {
    pub fn new(language: Arc<dyn Language>, ecosystem: &'static Ecosystem) -> Self {
        Self {
            language,
            ecosystem,
        }
    }
}

impl Producer<ProjectInfo> for ProjectRoots {
    fn id(&self) -> ProducerId {
        producer_id(self.language.as_ref())
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.language.id()])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<ProjectInfo> {
        ctx.cancel.checkpoint()?;
        let lang = self.language.id();
        let roots = roots(&ctx.ctx.root, self.ecosystem, &ctx.ctx.exclude)
            .into_iter()
            .filter(|root| root.lang == lang)
            .collect();
        Ok(ProjectInfo { roots })
    }
}
