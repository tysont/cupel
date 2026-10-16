// ABOUTME: Discovery (ADR-088): `[plugins]` entries, then `cupel-check-*`/`cupel-produce-*` executables on PATH,
// ABOUTME: each asked to `describe` itself and validated; what passes registers beside the native checks and producers.
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::config::{ConfigError, PluginEntry};
use cupel_core::plugin::PluginManifest;
use cupel_core::{
    Cancellation, CheckId, CheckRegistry, Error, LangId, ProducerId, ProducerRegistry,
};

use crate::check::PluginCheck;
use crate::command::{PluginCommand, QUICK_LIMIT, ended};
use crate::{manifest, producer};

/// The name prefixes of plugins on `PATH`, with the manifest kind each must describe.
const PATH_PREFIXES: [(&str, &str); 2] =
    [("cupel-check-", "check"), ("cupel-produce-", "producer")];

/// What discovery reads: the configured entries, the `PATH` to search, and what is registered.
pub struct Discovery<'a> {
    /// The repository root, where plugins run and relative programs are found.
    pub root: &'a Path,
    /// `[plugins]`.
    pub entries: &'a BTreeMap<String, PluginEntry>,
    /// The invocation's `PATH`; `None` searches nothing (`--no-path-plugins`, or no `PATH`).
    pub path: Option<&'a str>,
    /// The registered languages a producer plugin may name.
    pub languages: &'a [LangId],
    /// The check IDs already registered, which a check plugin may not reuse.
    pub taken: Vec<CheckId>,
    /// The run's cancellation; each `describe` also has its own limit.
    pub cancel: &'a Cancellation,
}

/// The plugins one invocation found, in discovery order, each with what it described.
#[derive(Debug, Clone, Default)]
pub struct Plugins {
    described: Vec<(PluginCommand, PluginManifest)>,
}

impl Plugins {
    pub fn is_empty(&self) -> bool {
        self.described.is_empty()
    }

    /// Each plugin's name, in discovery order.
    pub fn names(&self) -> Vec<String> {
        self.described
            .iter()
            .map(|(command, _)| command.name.clone())
            .collect()
    }

    /// The IDs of the check plugins, in discovery order.
    pub fn check_ids(&self) -> Vec<CheckId> {
        self.described
            .iter()
            .filter_map(|(_, manifest)| match manifest {
                PluginManifest::Check(manifest) => Some(manifest.id.clone()),
                PluginManifest::Producer(_) => None,
            })
            .collect()
    }

    /// Registers each check plugin after what `registry` holds (ADR-088 §7).
    ///
    /// # Panics
    ///
    /// When `registry` already holds a check a plugin describes; discovery refuses those, given
    /// the registry's IDs as `taken`.
    pub fn register_checks(&self, registry: &mut CheckRegistry) {
        for (command, manifest) in &self.described {
            if let PluginManifest::Check(manifest) = manifest {
                registry
                    .register(Box::new(PluginCheck::new(
                        manifest.clone(),
                        command.clone(),
                    )))
                    .expect("discovery admits only check IDs not already registered");
            }
        }
    }

    /// Registers each producer plugin after what `registry` holds (ADR-088 §7).
    pub fn register_producers(&self, registry: &mut ProducerRegistry) {
        for (command, manifest) in &self.described {
            if let PluginManifest::Producer(manifest) = manifest {
                producer::register(registry, manifest.clone(), command.clone());
            }
        }
    }
}

/// Finds, describes, and validates every plugin `request` names (ADR-088).
pub fn discover(request: &Discovery<'_>) -> Result<Plugins, ConfigError> {
    let mut admitted = Admitted {
        checks: request.taken.iter().cloned().collect(),
        producers: BTreeSet::new(),
        plugins: Plugins::default(),
    };
    let mut configured = BTreeSet::new();
    for (name, entry) in request.entries {
        let command = PluginCommand::new(name, entry.program(), entry.args(), request.root);
        if let Ok(file) = command.program().canonicalize() {
            configured.insert(file);
        }
        admitted.add(command, None, request)?;
    }
    for (name, file, kind) in on_path(request.path.unwrap_or_default(), &configured) {
        let command = PluginCommand::new(&name, &file.to_string_lossy(), &[], request.root);
        admitted.add(command, Some(kind), request)?;
    }
    Ok(admitted.plugins)
}

/// What discovery has admitted so far, and the IDs taken.
struct Admitted {
    checks: BTreeSet<CheckId>,
    producers: BTreeSet<ProducerId>,
    plugins: Plugins,
}

impl Admitted {
    /// Describes and admits the plugin `command` runs; `kind` is what its `PATH` name promises.
    fn add(
        &mut self,
        command: PluginCommand,
        kind: Option<&str>,
        request: &Discovery<'_>,
    ) -> Result<(), ConfigError> {
        let admitted = describe(&command, request.cancel).and_then(|manifest| {
            self.admit(&manifest, kind, request.languages)
                .map(|()| manifest)
        });
        match admitted {
            Ok(manifest) => {
                self.plugins.described.push((command, manifest));
                Ok(())
            }
            Err(problem) => Err(ConfigError::Plugin {
                name: command.name.clone(),
                program: command.display(),
                problem,
                on_path: kind.is_some(),
            }),
        }
    }

    fn admit(
        &mut self,
        manifest: &PluginManifest,
        kind: Option<&str>,
        languages: &[LangId],
    ) -> Result<(), String> {
        let described = match manifest {
            PluginManifest::Check(_) => "check",
            PluginManifest::Producer(_) => "producer",
        };
        if let Some(kind) = kind.filter(|kind| *kind != described) {
            let prefix = if kind == "check" {
                "cupel-check-"
            } else {
                "cupel-produce-"
            };
            return Err(format!(
                "a `{prefix}` plugin must describe a {kind}, not a {described}"
            ));
        }
        match manifest {
            PluginManifest::Check(manifest) => {
                manifest::check(manifest)?;
                unique(&mut self.checks, manifest.id.clone(), "check")
            }
            PluginManifest::Producer(manifest) => {
                manifest::producer(manifest, languages)?;
                unique(&mut self.producers, manifest.id.clone(), "producer")
            }
        }
    }
}

fn unique<T: Ord + std::fmt::Display>(
    taken: &mut BTreeSet<T>,
    id: T,
    what: &str,
) -> Result<(), String> {
    let message = format!("{what} `{id}` is already registered");
    if taken.insert(id) {
        Ok(())
    } else {
        Err(message)
    }
}

/// Runs `<plugin> describe` and reads its manifest.
fn describe(command: &PluginCommand, cancel: &Cancellation) -> Result<PluginManifest, String> {
    let captured = command
        .call("describe", None, &cancel.child(QUICK_LIMIT))
        .map_err(|error| match error {
            Error::Cancelled => format!(
                "`describe` did not answer within {} s",
                QUICK_LIMIT.as_secs()
            ),
            other => other.to_string(),
        })?;
    if captured.exit_code != Some(0) {
        return Err(format!("`describe` {}", ended(&captured, "")));
    }
    serde_json::from_str(&captured.stdout)
        .map_err(|error| format!("`describe` did not print a manifest: {error}"))
}

/// The plugins on `path`: executables named for a prefix, by name within a directory, the first
/// directory's winning, without those a `[plugins]` entry already runs (`configured`).
fn on_path(path: &str, configured: &BTreeSet<PathBuf>) -> Vec<(String, PathBuf, &'static str)> {
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    for dir in std::env::split_paths(path) {
        for (name, file, kind) in plugins_in(&dir) {
            if !seen.insert(name.clone()) {
                continue;
            }
            let duplicate = file
                .canonicalize()
                .is_ok_and(|real| configured.contains(&real));
            if !duplicate {
                found.push((name, file, kind));
            }
        }
    }
    found
}

/// The executables in `dir` named for a plugin prefix, sorted by name.
fn plugins_in(dir: &Path) -> Vec<(String, PathBuf, &'static str)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut plugins: Vec<_> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let (_, kind) = PATH_PREFIXES
                .iter()
                .find(|(prefix, _)| name.len() > prefix.len() && name.starts_with(prefix))?;
            let file = entry.path();
            executable(&file).then_some((name, file, *kind))
        })
        .collect();
    plugins.sort();
    plugins
}

#[cfg(unix)]
fn executable(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(file).is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn executable(file: &Path) -> bool {
    file.is_file()
}
