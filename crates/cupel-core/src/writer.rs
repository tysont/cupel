// ABOUTME: `Writer`, the only way Cupel writes files: free under `<root>/.cupel/`, elsewhere only with
// ABOUTME: `--write`, judged where a path really lands; a recording writer plans diffs instead (ADR-020).
use std::fs::{self, File};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use crate::dry_run::PlannedWrite;
use crate::envelope::Envelope;
use crate::error::{Error, Result};
use crate::id::RunId;

/// Why a write was not made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum WriteError {
    /// The path lands outside the writer's area and `--write` was not given.
    #[error("`{}` is outside .cupel/; pass --write to permit writing it", .path.display())]
    Outside { path: PathBuf },
    /// The file system refused.
    #[error("cannot write `{}`: {message}", .path.display())]
    Io { path: PathBuf, message: String },
}

impl WriteError {
    fn io(path: &Path, error: &std::io::Error) -> Error {
        Error::Write(Self::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })
    }
}

/// Writes files for one run. Cheap to clone; clones share a dry run's recorded writes.
#[derive(Debug, Clone)]
pub struct Writer {
    /// The canonical repository root.
    root: PathBuf,
    /// Writes inside this directory are always allowed.
    area: PathBuf,
    /// `--write`: writes outside `area` are allowed too.
    outside: bool,
    /// Present for a dry run: writes are recorded here instead of made.
    recorded: Option<Arc<Mutex<Vec<PlannedWrite>>>>,
}

impl Writer {
    /// The directory, relative to the root, that holds everything Cupel keeps.
    pub const DIR: &'static str = ".cupel";
    /// The content of `.cupel/.gitignore`: ignore everything but suppressions (design §8.8).
    pub const GITIGNORE: &'static str = "*\n!suppressions.toml\n";

    /// A writer for the repository at `root`; `write` is the `--write` flag.
    pub fn new(root: &Path, write: bool) -> Result<Self> {
        let root = root
            .canonicalize()
            .map_err(|error| WriteError::io(root, &error))?;
        Ok(Self {
            area: root.join(Self::DIR),
            root,
            outside: write,
            recorded: None,
        })
    }

    /// This writer as a dry run: every write is checked as usual, then recorded as a
    /// [`PlannedWrite`] instead of made.
    #[must_use]
    pub fn recording(self) -> Self {
        Self {
            recorded: Some(Arc::default()),
            ..self
        }
    }

    /// A writer that never writes outside `.cupel/`, whatever `--write` said. Producers get this.
    #[must_use]
    pub fn confined(&self) -> Self {
        Self {
            outside: false,
            ..self.clone()
        }
    }

    /// A writer confined to `run`'s scratch directory, `.cupel/work/<run>/`. Checks get this.
    #[must_use]
    pub fn scratch(&self, run: &RunId) -> Self {
        Self {
            area: self.work_dir(run),
            outside: false,
            ..self.clone()
        }
    }

    /// Whether this is a dry run's writer, which records writes instead of making them.
    pub fn is_recording(&self) -> bool {
        self.recorded.is_some()
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory this writer may always write in.
    pub fn area(&self) -> &Path {
        &self.area
    }

    pub fn cupel_dir(&self) -> PathBuf {
        self.root.join(Self::DIR)
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.cupel_dir().join("cache")
    }

    pub fn work_dir(&self, run: &RunId) -> PathBuf {
        self.cupel_dir().join("work").join(run.to_string())
    }

    pub fn run_record_path(&self, run: &RunId) -> PathBuf {
        self.cupel_dir().join("runs").join(format!("{run}.json"))
    }

    /// Writes `envelope` as the run record `.cupel/runs/<id>.json`: pretty JSON and a newline.
    pub fn run_record(&self, envelope: &Envelope) -> Result<PathBuf> {
        let path = self.run_record_path(&envelope.run.id);
        let mut json = serde_json::to_string_pretty(envelope).map_err(|error| {
            Error::Write(WriteError::Io {
                path: path.clone(),
                message: error.to_string(),
            })
        })?;
        json.push('\n');
        self.write(&path, json.as_bytes())
    }

    /// Writes `.cupel/.gitignore`.
    pub fn gitignore(&self) -> Result<PathBuf> {
        self.write(
            &self.cupel_dir().join(".gitignore"),
            Self::GITIGNORE.as_bytes(),
        )
    }

    /// Writes `contents` to `path` (relative to the root, or absolute), creating parent
    /// directories. Returns where the file landed.
    pub fn write(&self, path: &Path, contents: &[u8]) -> Result<PathBuf> {
        let target = self.permitted(path)?;
        if let Some(recorded) = &self.recorded {
            let planned = self.planned_write(&target, contents);
            recorded
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(planned);
            return Ok(target);
        }
        self.create_parent(&target)?;
        fs::write(&target, contents).map_err(|error| WriteError::io(&target, &error))?;
        Ok(target)
    }

    /// Creates the directory `path` and its parents; a dry run only checks it.
    pub fn create_dir(&self, path: &Path) -> Result<PathBuf> {
        let target = self.permitted(path)?;
        if self.recorded.is_none() {
            self.ensure_cupel_dir(&target)?;
            fs::create_dir_all(&target).map_err(|error| WriteError::io(&target, &error))?;
        }
        Ok(target)
    }

    /// Proves this writer's area can be written: creates it if needed (with `.cupel/.gitignore`),
    /// then creates and removes a uniquely named file in it. A recording writer touches nothing.
    pub fn probe(&self) -> Result<()> {
        if self.recorded.is_some() {
            return Ok(());
        }
        let probe = self.area.join(format!(".probe-{}", RunId::generate()));
        self.create_parent(&probe)?;
        File::create_new(&probe).map_err(|error| WriteError::io(&probe, &error))?;
        fs::remove_file(&probe).map_err(|error| WriteError::io(&probe, &error))
    }

    /// Opens `path` for streaming output, truncating it. A stream is an output channel the user
    /// asked for (such as `--events`), so it is opened even by a recording writer.
    pub fn stream(&self, path: &Path) -> Result<File> {
        let target = self.permitted(path)?;
        self.create_parent(&target)?;
        File::create(&target).map_err(|error| WriteError::io(&target, &error))
    }

    /// The writes a recording writer has recorded, in order; empty otherwise.
    pub fn planned(&self) -> Vec<PlannedWrite> {
        self.recorded
            .as_ref()
            .map(|recorded| {
                recorded
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone()
            })
            .unwrap_or_default()
    }

    /// Where `path` really lands, if this writer may write there.
    fn permitted(&self, path: &Path) -> Result<PathBuf> {
        let target = self.resolve(path)?;
        if target.starts_with(&self.area) || self.outside {
            Ok(target)
        } else {
            Err(Error::Write(WriteError::Outside { path: target }))
        }
    }

    /// Where `path` (relative to the root, or absolute) really lands. Components are followed
    /// one at a time: an existing one is canonicalized, so symlinks are followed; a missing one
    /// is appended as written, and `..` steps back to the parent. A dangling symlink resolves
    /// nowhere and is refused.
    fn resolve(&self, path: &Path) -> Result<PathBuf> {
        let joined = self.root.join(path);
        let mut resolved = PathBuf::new();
        for component in joined.components() {
            match component {
                Component::Prefix(_) | Component::RootDir => resolved.push(component),
                Component::CurDir => {}
                Component::ParentDir => {
                    resolved.pop();
                }
                Component::Normal(part) => {
                    resolved.push(part);
                    if fs::symlink_metadata(&resolved).is_ok() {
                        resolved = resolved.canonicalize().map_err(|_| {
                            Error::Write(WriteError::Outside {
                                path: joined.clone(),
                            })
                        })?;
                    }
                }
            }
        }
        Ok(resolved)
    }

    fn create_parent(&self, target: &Path) -> Result<()> {
        self.ensure_cupel_dir(target)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| WriteError::io(parent, &error))?;
        }
        Ok(())
    }

    /// Creates `.cupel/` with its `.gitignore` before the first write into it, so run records
    /// never show up in `git status` (ADR-020 §2).
    fn ensure_cupel_dir(&self, target: &Path) -> Result<()> {
        let dir = self.cupel_dir();
        if !target.starts_with(&dir) || dir.exists() {
            return Ok(());
        }
        fs::create_dir_all(&dir).map_err(|error| WriteError::io(&dir, &error))?;
        let gitignore = dir.join(".gitignore");
        fs::write(&gitignore, Self::GITIGNORE).map_err(|error| WriteError::io(&gitignore, &error))
    }

    fn planned_write(&self, target: &Path, contents: &[u8]) -> PlannedWrite {
        let shown = target.strip_prefix(&self.root).unwrap_or(target);
        let shown = shown.display().to_string();
        let current = fs::read(target).ok();
        let old = current.as_deref().map(String::from_utf8_lossy);
        let new = String::from_utf8_lossy(contents);
        PlannedWrite::change(&shown, &shown, old.as_deref(), &new)
    }
}
