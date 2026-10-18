// ABOUTME: The fuzz producers (ADR-107): one `Producer<FuzzResults>` per fuzzer, each finding a language's fuzz targets in
// ABOUTME: the project roots in scope and fuzzing each once with the run's seed and an execution budget, outside the tree.
//! The fuzz producers.
//!
//! Each fuzzer ([`cargo_fuzz`], [`go`], [`atheris`], [`jazzer`]) is a [`Fuzzer`]: it lists a
//! project root's targets and fuzzes one target, and [`FuzzProducer`] does the rest the same
//! way for all of them: which roots are in scope, the [`Budget`] (executions from
//! `Focus::case_budget`, the seed, a per-input limit, a wall-clock cap), where corpora and
//! crash inputs go (`.cupel/cache/fuzz/<lang>/<root>/<target>/`, emptied before each run so a
//! seed repeats a run), and reading a saved input back as text. The libFuzzer-based fuzzers
//! share [`libfuzzer`]'s arguments and output reader.

mod atheris;
pub(crate) mod cargo_fuzz;
mod go;
mod jazzer;
mod libfuzzer;

use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::{
    Availability, CostClass, Ctx, Error, EventSink, Exclude, FactKind, FuzzCrash, FuzzResults,
    FuzzStatus, FuzzTarget, LangId, Langs, Location, ProduceCtx, Producer, ProducerId, Result,
    Scope, ToolProbe, output_tail,
};

use crate::analyze::extensions;
use crate::runner::suite_roots;

/// The longest any one input may run before the fuzzer calls it a hang, in seconds.
pub const INPUT_SECONDS: u64 = 10;

/// The longest one target is fuzzed, in seconds, whatever its execution budget.
pub const TARGET_SECONDS: u64 = 300;

/// How much one target is fuzzed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// Executions (`--cases`, else by depth).
    pub cases: u64,
    /// The run's seed.
    pub seed: u64,
}

impl Budget {
    /// The seed as libFuzzer takes it: a nonzero 32-bit number (0 asks it for a random seed).
    /// A seed from 1 to 2³²−1 is itself; any other is its remainder by 2³²−1, plus one.
    pub fn libfuzzer_seed(&self) -> u32 {
        u32::try_from(self.seed)
            .ok()
            .filter(|seed| *seed != 0)
            .unwrap_or_else(|| u32::try_from(self.seed % u64::from(u32::MAX)).unwrap_or(0) + 1)
    }
}

/// A fuzz target a fuzzer found in a project root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// The target as the fuzzer names it.
    pub name: String,
    /// The file that defines it, relative to the project root.
    pub file: Option<String>,
    pub line: u32,
}

/// One project root being fuzzed.
pub struct Root<'a> {
    pub ctx: &'a ProduceCtx,
    /// Relative to the repository root; `.` for the root itself.
    pub dir: String,
    /// The project root on disk.
    pub path: PathBuf,
    /// The root's package patterns (`<packages>`), as `ProjectInfo` gives them.
    pub packages: Vec<String>,
    pub budget: Budget,
    /// `.cupel/cache/fuzz/<lang>/<root>/`.
    pub cache: PathBuf,
}

/// What a target's run left: how it ended, its output, and the saved failing input.
pub struct Ended {
    pub status: FuzzStatus,
    pub output: String,
    /// The file holding the failing input, when the fuzzer saved one.
    pub artifact: Option<PathBuf>,
    pub executions: Option<u64>,
    pub coverage: Option<u64>,
}

/// One fuzzer: how to find a root's targets and fuzz one of them.
pub trait Fuzzer: Send + Sync + 'static {
    /// The producer's ID, such as `tool.cargo-fuzz`.
    fn id(&self) -> &'static str;
    fn lang(&self) -> &'static str;
    /// Whether it takes the run's seed.
    fn seeded(&self) -> bool;
    fn availability(&self, ctx: &Ctx) -> Availability;
    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe>;
    /// The root's targets; `None` when the root is not one the fuzzer searches (a cargo-fuzz
    /// crate is a project root of its own, but its targets belong to the crate it fuzzes).
    fn targets(&self, root: &Root<'_>) -> Result<Option<Vec<Found>>>;
    /// Fuzzes `target` with the root's budget, its corpus and saved inputs under `area`.
    fn fuzz(&self, root: &Root<'_>, target: &Found, area: &Path) -> Result<Ended>;
    /// What a failed run reported, and where in the repository it failed.
    fn failure(&self, root: &Root<'_>, output: &str) -> (String, Option<Location>);
    /// The command, run in the project root, that runs `target` on the saved input `input`
    /// (relative to the project root).
    fn reproduce(&self, root: &Root<'_>, target: &Found, input: &str) -> String;
    /// The saved input in `artifact`, as the fact shows it ([`input_text`] by default).
    fn input(&self, artifact: &Path) -> String {
        input_text(artifact)
    }
}

/// A fuzzer as a producer of `FuzzResults`.
pub struct FuzzProducer<F>(pub F);

impl<F: Fuzzer> Producer<FuzzResults> for FuzzProducer<F> {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id())
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from(self.0.lang())])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    fn availability(&self, ctx: &Ctx) -> Availability {
        self.0.availability(ctx)
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        self.0.tool(ctx)
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<FuzzResults> {
        let budget = Budget {
            cases: u64::from(ctx.focus.case_budget()),
            seed: ctx.seed,
        };
        let mut results = FuzzResults {
            tool: self.id(),
            seed: ctx.seed,
            seeded: self.0.seeded(),
            cases: budget.cases,
            roots: Vec::new(),
            targets: Vec::new(),
        };
        let lang = LangId::from(self.0.lang());
        for (dir, packages) in suite_roots(ctx) {
            if !in_scope(scope, &dir, extensions(&lang)) {
                continue;
            }
            ctx.cancel.checkpoint()?;
            let root = Root {
                ctx,
                path: ctx.ctx.root.join(&dir),
                cache: ctx
                    .writer
                    .cache_dir()
                    .join("fuzz")
                    .join(self.0.lang())
                    .join(slug(&dir)),
                dir,
                packages,
                budget,
            };
            let Some(found) = self.0.targets(&root)? else {
                continue;
            };
            let found = outside_excluded(&ctx.ctx.exclude, &root.dir, found);
            results.roots.push(root.dir.clone());
            for target in &found {
                ctx.cancel.checkpoint()?;
                results.targets.push(self.fuzzed(&root, target)?);
            }
        }
        Ok(results)
    }
}

impl<F: Fuzzer> FuzzProducer<F> {
    /// Fuzzes one target in a fresh area and reads back how it ended.
    fn fuzzed(&self, root: &Root<'_>, target: &Found) -> Result<FuzzTarget> {
        let area = fresh_area(root, &target.name)?;
        let ended = self.0.fuzz(root, target, &area)?;
        let failed = matches!(ended.status, FuzzStatus::Crashed | FuzzStatus::TimedOut);
        let crash = failed.then(|| self.crash(root, target, &ended));
        Ok(FuzzTarget {
            root: root.dir.clone(),
            name: target.name.clone(),
            location: target.file.as_ref().map(|file| Location {
                file: crate::tree::under(&root.dir, file),
                line: target.line,
                col: None,
                end_line: target.line,
                end_col: None,
            }),
            status: ended.status,
            detail: (ended.status == FuzzStatus::Failed)
                .then(|| output_tail(&ended.output).unwrap_or_else(|| "no output".to_owned())),
            executions: ended.executions,
            coverage: ended.coverage,
            crash,
        })
    }

    fn crash(&self, root: &Root<'_>, target: &Found, ended: &Ended) -> FuzzCrash {
        let (message, location) = self.0.failure(root, &ended.output);
        let artifact = ended.artifact.as_deref();
        FuzzCrash {
            message,
            location,
            input: artifact.map(|path| self.0.input(path)).unwrap_or_default(),
            reproducer: artifact.map(|path| relative(&root.ctx.ctx.root, path)),
            reproduce: artifact
                .map(|path| self.0.reproduce(root, target, &relative(&root.path, path))),
        }
    }
}

/// Whether the project root `dir` holds a file of the scope with one of `extensions`; every
/// root is, for an empty scope.
pub(crate) fn in_scope(scope: &Scope, dir: &str, extensions: &[&str]) -> bool {
    scope.files.is_empty()
        || scope.files.iter().any(|file| {
            let path = Path::new(file);
            (dir == "." || path.starts_with(dir))
                && path
                    .extension()
                    .is_some_and(|ext| extensions.iter().any(|known| ext == *known))
        })
}

/// `targets` of the project root `dir` less those defined in a file `[scope] exclude` leaves
/// out of the run (a fixture, vendored code); a target placed nowhere stays.
fn outside_excluded(exclude: &Exclude, dir: &str, targets: Vec<Found>) -> Vec<Found> {
    targets
        .into_iter()
        .filter(|target| {
            target
                .file
                .as_ref()
                .is_none_or(|file| !exclude.excludes(&crate::tree::under(dir, file)))
        })
        .collect()
}

/// Directories never searched for targets or copied: version control, Cupel's own, and
/// dependencies and build outputs that hold other projects' code.
const SKIPPED: [&str; 5] = [".git", ".cupel", "node_modules", "target", "__pycache__"];

/// The files under `dir` (outside [`SKIPPED`] directories and what `.gitignore` ignores) that
/// `keep` accepts given their path relative to `dir` and on disk, by path.
pub fn sources(dir: &Path, keep: impl Fn(&str, &Path) -> bool) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = walk(dir)
        .filter_map(|path| {
            let relative = path
                .strip_prefix(dir)
                .ok()?
                .to_string_lossy()
                .replace('\\', "/");
            keep(&relative, &path).then_some((relative, path))
        })
        .collect();
    found.sort();
    found
}

/// Every file under `dir` outside [`SKIPPED`] directories and what `.gitignore` ignores.
pub fn walk(dir: &Path) -> impl Iterator<Item = PathBuf> {
    ignore::WalkBuilder::new(dir)
        .hidden(false)
        .require_git(false)
        .filter_entry(|entry| {
            !SKIPPED
                .iter()
                .any(|skipped| entry.file_name() == std::ffi::OsStr::new(skipped))
        })
        .build()
        .flatten()
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_file()))
        .map(ignore::DirEntry::into_path)
}

/// A directory name for the project root `dir`.
fn slug(dir: &str) -> String {
    if dir == "." {
        "root".to_owned()
    } else {
        dir.replace('/', "_")
    }
}

/// The last count after `marker` in `output` (libFuzzer's ` cov: 12`, Go's `execs: 1000`).
pub fn last_count(output: &str, marker: &str) -> Option<u64> {
    output.lines().rev().find_map(|line| {
        let (_, after) = line.split_once(marker)?;
        after.split_whitespace().next()?.parse().ok()
    })
}

/// `.cupel/cache/fuzz/<lang>/<root>/<target>/`, emptied, with its `corpus/` and `inputs/`.
fn fresh_area(root: &Root<'_>, target: &str) -> Result<PathBuf> {
    let area = root.cache.join(target.replace(['/', '\\', ':'], "_"));
    if area.exists() {
        fs::remove_dir_all(&area).map_err(|error| Error::Process {
            program: "fuzz".to_owned(),
            message: format!("cannot empty {}: {error}", area.display()),
        })?;
    }
    root.ctx.writer.create_dir(&area.join("corpus"))?;
    root.ctx.writer.create_dir(&area.join("inputs"))?;
    Ok(area)
}

/// `path` relative to `base` when it is under it (both made canonical), else as given.
pub fn relative(base: &Path, path: &Path) -> String {
    let canonical = |path: &Path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let (base, full) = (canonical(base), canonical(path));
    full.strip_prefix(&base)
        .map_or_else(|_| full.clone(), Path::to_path_buf)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Whether `path` has the extension `extension`, in any case.
pub fn extension_is(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
}

/// The longest input shown, in characters, or bytes for one shown as hex.
const SHOWN: usize = 256;

/// The input saved in `path`: its text when it is printable UTF-8, else `hex:` and its bytes;
/// at most [`SHOWN`] characters of it.
pub fn input_text(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_default();
    shown(&bytes)
}

/// `bytes` as [`input_text`] shows them.
pub fn shown(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text)
            if !text
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t')) =>
        {
            text.chars().take(SHOWN).collect()
        }
        _ => {
            let hex = bytes
                .iter()
                .take(SHOWN / 2)
                .fold(String::new(), |mut hex, byte| {
                    use std::fmt::Write as _;
                    let _ = write!(hex, "{byte:02x}");
                    hex
                });
            format!("hex:{hex}")
        }
    }
}

/// The location of `line` (and `col`) of `file`, a path as a fuzzer printed it, when it is in the
/// repository at `root`.
pub fn located(root: &Path, file: &str, line: u32, col: Option<u32>) -> Option<Location> {
    let path = Path::new(file);
    let full = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    let canonical_root = root.canonicalize().ok()?;
    let relative = full
        .canonicalize()
        .ok()?
        .strip_prefix(&canonical_root)
        .ok()?
        .to_path_buf();
    Some(Location {
        file: relative.to_string_lossy().replace('\\', "/"),
        line,
        col,
        end_line: line,
        end_col: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_seed_reaches_libfuzzer_as_a_nonzero_32_bit_number() {
        let seed = |seed| Budget { cases: 1, seed }.libfuzzer_seed();
        assert_eq!(seed(3), 3);
        assert_eq!(seed(u64::from(u32::MAX)), u32::MAX);
        assert_eq!(seed(0), 1);
        assert_eq!(seed(u64::from(u32::MAX) + 5), 6);
    }

    #[test]
    fn a_printable_input_is_its_text_and_any_other_is_hex() {
        assert_eq!(shown(b""), "");
        assert_eq!(shown(b"FUZ\n"), "FUZ\n");
        assert_eq!(shown(&[0xff, 0x00, 0x41]), "hex:ff0041");
        assert_eq!(shown("x".repeat(300).as_bytes()).len(), SHOWN);
        assert_eq!(shown(&[0u8; 300]).len(), "hex:".len() + SHOWN);
    }

    #[test]
    fn a_root_is_in_scope_when_it_holds_a_scoped_file_of_the_language() {
        let scope = Scope::of_files(vec!["tools/a.rs".to_owned(), "web/x.js".to_owned()]);
        let kept = [
            in_scope(&scope, "tools", &["rs"]),
            in_scope(&scope, "web", &["rs"]),
            in_scope(&scope, ".", &["js"]),
            in_scope(&Scope::default(), "anything", &["rs"]),
        ];
        assert_eq!(kept, [true, false, true, true]);
    }

    #[test]
    fn a_target_defined_in_an_excluded_file_is_not_fuzzed() {
        let found = |name: &str, file: Option<&str>| Found {
            name: name.to_owned(),
            file: file.map(str::to_owned),
            line: 1,
        };
        let targets = vec![
            found("kept", Some("fuzz/first.fuzz.js")),
            found("left", Some("fixtures/first.fuzz.js")),
            found("unplaced", None),
        ];
        let names = |pattern: &str, dir: &str| -> Vec<String> {
            let exclude = Exclude::new(vec![pattern.to_owned()]).unwrap();
            outside_excluded(&exclude, dir, targets.clone())
                .into_iter()
                .map(|target| target.name)
                .collect()
        };
        assert_eq!(
            [
                names("fixtures/**", "."),
                names("web/fixtures/**", "web"),
                names("vendor-fixtures/**", "."),
            ],
            [
                vec!["kept", "unplaced"],
                vec!["kept", "unplaced"],
                vec!["kept", "left", "unplaced"],
            ]
        );
    }

    #[test]
    fn only_a_place_inside_the_repository_is_located() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir(repo.path().join("src")).unwrap();
        fs::write(repo.path().join("src/lib.rs"), "").unwrap();
        let absolute = repo.path().join("src/lib.rs");
        assert_eq!(
            located(repo.path(), &absolute.to_string_lossy(), 5, Some(3)).map(|at| at.file),
            Some("src/lib.rs".to_owned())
        );
        assert_eq!(
            located(repo.path(), "src/lib.rs", 5, None).map(|at| at.line),
            Some(5)
        );
        assert_eq!(located(repo.path(), "/usr/lib/x.rs", 1, None), None);
    }
}
