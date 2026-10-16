// ABOUTME: The `oracle` verb (ADR-090): `add` and `remove` edit cupel.toml (previewed without --write),
// ABOUTME: `list` reports each oracle's state, `verify` runs smoke comparisons; plus what `check` reads.
mod cap;
mod edit;
mod select;
mod verify;

use std::path::Path;

use cupel_core::config::{Config, OracleConfig, OracleKind};
use cupel_core::git::Git;
use cupel_core::{
    ActionKind, Envelope, EventSink, ExitCode, OracleReport, OracleState, Status,
    oracle_content_digest, oracle_identity,
};
use cupel_store::Store;

pub use cap::{cap, finding_kinds};
pub use select::Selected;

use crate::cli::OracleArgs;
use crate::error::{CupelError, Usage};
use crate::persist;
use crate::run::Invocation;

/// Runs `action`. A dry run of `verify` runs nothing and plans nothing.
pub fn run(
    invocation: &Invocation<'_>,
    action: &OracleArgs,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let config = invocation.config()?;
    match action {
        OracleArgs::List => {
            let names: Vec<&String> = config.file.oracles.keys().collect();
            let store = persist::open_for_reading(invocation.writer)?;
            let reports = reports(invocation, config, store.as_ref(), &names)?;
            let mut envelope = invocation
                .run
                .reported(Status::Pass, ExitCode::Pass, Vec::new());
            envelope.oracles = Some(reports);
            Ok(envelope)
        }
        OracleArgs::Add { name, .. } => {
            edit::add(invocation, config, name, &definition(action), events)
        }
        OracleArgs::Remove { name } => edit::remove(invocation, config, name, events),
        OracleArgs::Verify { name, all } => {
            let names = verified_names(config, name.as_deref(), *all)?;
            if invocation.dry_run {
                return Ok(invocation.nothing_planned(events));
            }
            verify::run(invocation, config, &names)
        }
    }
}

/// The definition `oracle add` was given.
fn definition(action: &OracleArgs) -> OracleConfig {
    let OracleArgs::Add {
        kind,
        reference,
        cmd,
        spec,
        lang,
        inputs,
        ..
    } = action
    else {
        unreachable!("only `oracle add` carries a definition")
    };
    OracleConfig {
        kind: match kind {
            crate::cli::OracleKind::Ref => OracleKind::Ref,
            crate::cli::OracleKind::Command => OracleKind::Command,
            crate::cli::OracleKind::Spec => OracleKind::Spec,
        },
        reference: reference.clone(),
        cmd: cmd.clone(),
        spec: spec.as_ref().map(|spec| spec.display().to_string()),
        lang: lang.clone(),
        inputs: inputs.clone(),
    }
}

/// The oracles `verify` names: every one with `--all`, else `name`, which must be defined.
fn verified_names<'c>(
    config: &'c Config,
    name: Option<&str>,
    all: bool,
) -> Result<Vec<&'c String>, CupelError> {
    match name {
        Some(name) if !all => Ok(vec![defined(config, name, "NAME")?.0]),
        _ => Ok(config.file.oracles.keys().collect()),
    }
}

/// The oracle `name` and its definition; a usage error on `argument` listing the defined
/// oracles when there is none.
pub(crate) fn defined<'c>(
    config: &'c Config,
    name: &str,
    argument: &str,
) -> Result<(&'c String, &'c OracleConfig), CupelError> {
    config.file.oracles.get_key_value(name).ok_or_else(|| {
        let valid: Vec<String> = config.file.oracles.keys().cloned().collect();
        CupelError::from(Usage {
            subject: argument.to_owned(),
            title: format!("No oracle named `{name}` is defined"),
            fix: "Name an oracle defined in cupel.toml (`cupel oracle list`), or add one with \
                  `cupel oracle add`."
                .to_owned(),
            detail: None,
            argument: Some(argument.to_owned()),
            value: Some(name.to_owned()),
            valid,
        })
    })
}

/// Why an oracle cannot be reached now, and what to do about it.
pub(crate) struct Unreachable {
    pub title: String,
    pub action: ActionKind,
    pub fix: String,
}

impl Unreachable {
    fn configure(title: String, fix: &str) -> Self {
        Self {
            title,
            action: ActionKind::Configure,
            fix: fix.to_owned(),
        }
    }
}

/// What `definition` resolves to in the repository at `root` now (ADR-090 §4): the commit of a
/// ref, the digest of a spec file, nothing for a command.
pub(crate) fn resolve(
    root: &Path,
    definition: &OracleConfig,
) -> Result<Option<String>, Unreachable> {
    let given = |value: &Option<String>| value.clone().unwrap_or_default();
    match definition.kind {
        OracleKind::Command => Ok(None),
        OracleKind::Ref => {
            let reference = given(&definition.reference);
            let git = Git::open(root).ok_or_else(|| {
                Unreachable::configure(
                    format!("`{reference}` cannot be resolved outside a git work tree"),
                    "Run in the git work tree whose history holds the ref.",
                )
            })?;
            git.commit(&reference).map(Some).map_err(|error| {
                Unreachable::configure(
                    error.message,
                    "Point `ref` at an existing branch, tag, or commit, or fetch it.",
                )
            })
        }
        OracleKind::Spec => {
            let spec = given(&definition.spec);
            std::fs::read(root.join(&spec))
                .map(|bytes| Some(oracle_content_digest(&bytes)))
                .map_err(|error| {
                    Unreachable::configure(
                        format!("the spec file `{spec}` cannot be read: {error}"),
                        "Point `spec` at the specification file, relative to the repository root.",
                    )
                })
        }
    }
}

/// The identity of `definition` now; `None` when it cannot be reached.
pub(crate) fn identity(root: &Path, definition: &OracleConfig) -> Option<String> {
    resolve(root, definition)
        .ok()
        .map(|resolved| oracle_identity(definition, resolved.as_deref()))
}

/// The state of the oracle `name` now, against `store`'s verification of it.
pub(crate) fn report(
    root: &Path,
    store: Option<&Store>,
    name: &str,
    definition: &OracleConfig,
) -> Result<OracleReport, CupelError> {
    let recorded = store
        .map(|store| store.verified_oracle(name))
        .transpose()
        .map_err(|error| persist::unusable(&error))?
        .flatten();
    let hash = identity(root, definition);
    Ok(OracleReport {
        name: name.to_owned(),
        definition: definition.clone(),
        state: OracleState::of(
            hash.as_deref(),
            recorded.as_ref().map(|verified| verified.hash.as_str()),
        ),
        hash,
        verified_at: recorded
            .as_ref()
            .map(|verified| verified.verified_at.clone()),
        verified_run: recorded.map(|verified| verified.run),
    })
}

/// The reports of the oracles `names`, all defined in `config`.
fn reports(
    invocation: &Invocation<'_>,
    config: &Config,
    store: Option<&Store>,
    names: &[&String],
) -> Result<Vec<OracleReport>, CupelError> {
    names
        .iter()
        .map(|name| {
            let definition = &config.file.oracles[*name];
            report(invocation.writer.root(), store, name, definition)
        })
        .collect()
}
