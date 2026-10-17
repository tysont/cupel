// ABOUTME: The contract producers (ADR-113): one `ContractViolations` producer per contract library, each running the
// ABOUTME: baseline's runner once more with the library's contracts all on and properties at the budget, then reading failures.
//! The contract producers.
//!
//! Each contract library is a [`Library`]: its language, the dependency name its users' manifests
//! declare, the environment that turns on every contract it checks, and the reader of its
//! violations in one failed test's output. One [`Contracts`] producer per library needs the
//! green baseline's [`TestResults`] (ADR-076), which names the runner, and [`TestFrames`], whose
//! property cases get the run's seed and case budget as in the property engine (ADR-095). When
//! a project root declares the library it runs the suite again through that runner with the
//! switches on, and reads every failed test's output: a contract the project's own runs leave
//! unchecked, or one only a generated input reaches, now fails a test, and the library says
//! which contract it was.

mod icontract;
mod rust;

use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::syntax::{ProjectInfo, TestFrames};
use cupel_core::{
    Availability, ContractKind, ContractPlace, ContractViolation, ContractViolations, CostClass,
    Ctx, Error, EventSink, FactKind, LangId, Langs, ProduceCtx, Producer, ProducerId,
    ProducerRegistry, Result, RootRun, RunStatus, Scope, TestOutcome, TestResults, TestSelection,
};

use crate::rules::{Dirs, Rule};
use crate::tree::{canonical, relative};

/// One contract library.
pub struct Library {
    pub id: &'static str,
    pub lang: &'static str,
    /// The package name a project root's manifests declare to use it.
    pub package: &'static str,
    /// The globs of the manifests that declare it, in a root or a member directory.
    pub manifests: &'static [&'static str],
    /// The environment that turns on every contract it checks.
    pub switches: &'static [(&'static str, &'static str)],
    /// The violations in one failed test's output, each file as the library printed it; the
    /// function reads a file as printed, for a library whose report leaves the kind to its
    /// source.
    pub read: fn(&str, &Source<'_>) -> Vec<Found>,
}

/// Reads a file a library printed, as it printed it.
pub type Source<'a> = dyn Fn(&str) -> Option<String> + 'a;

/// One violation as a library reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub kind: ContractKind,
    pub function: Option<String>,
    pub message: String,
    /// As printed: absolute, or relative to the project root the runner ran in.
    pub file: Option<String>,
    pub line: u32,
}

/// The `ContractViolations` producer of one library.
pub struct Contracts(&'static Library);

fn register(registry: &mut ProducerRegistry, library: &'static Library) {
    registry.register::<ContractViolations, _>(Contracts(library));
}

impl Producer<ContractViolations> for Contracts {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from(self.0.lang)])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::TestResults, FactKind::TestFrames]
    }

    /// The project roots whose manifests may declare the library, as the baseline ran in them.
    fn wants(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Standard
    }

    /// Ready wherever the runner is: a project that declares the library cannot run its own
    /// tests without it, so a missing library is the baseline's failure, not this producer's.
    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<ContractViolations> {
        let Some(baseline) = ctx.upstream.get::<TestResults>() else {
            return Ok(self.nothing(self.id()));
        };
        let mut fact = self.nothing(baseline.runner.clone());
        fact.declared = declared(ctx, self.0);
        if !fact.declared {
            return Ok(fact);
        }
        let runner = ctx
            .runners
            .get(&baseline.runner)
            .ok_or_else(|| Error::InvalidValue {
                what: "runner for the contract run",
                value: baseline.runner.to_string(),
                expected: "a registered runner",
            })?;
        let mut options = crate::property::budgeted(
            ctx,
            &baseline.runner,
            ctx.upstream.get::<TestFrames>(),
            scope,
        );
        options.env.extend(
            self.0
                .switches
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned())),
        );
        let ran = runner.run(ctx, &TestSelection::All, &options)?;
        fact.runs = ran
            .results
            .invocations
            .iter()
            .map(|invocation| RootRun {
                dir: invocation.dir.clone(),
                status: if invocation.completed {
                    RunStatus::Ran
                } else {
                    RunStatus::Failed
                },
                detail: invocation.output.clone(),
            })
            .collect();
        fact.tests = ran
            .results
            .tests
            .iter()
            .filter(|test| test.outcome != TestOutcome::Skipped)
            .count() as u64;
        let root = canonical(&ctx.ctx.root);
        let dirs: Vec<PathBuf> = ran
            .results
            .invocations
            .iter()
            .map(|invocation| canonical(&root.join(&invocation.dir)))
            .collect();
        let source = |file: &str| fs::read_to_string(located(&dirs, file)?).ok();
        for test in ran.results.failures() {
            for found in (self.0.read)(test.output.as_deref().unwrap_or_default(), &source) {
                let violation = ContractViolation {
                    kind: found.kind,
                    function: found.function,
                    message: found.message,
                    place: found
                        .file
                        .and_then(|file| placed(&root, &dirs, &file))
                        .map(|file| ContractPlace {
                            file,
                            line: found.line,
                        }),
                    test: test.id.clone(),
                };
                let test_file = test.file.as_deref();
                if in_scope(&violation, test_file, scope) && !fact.violations.contains(&violation) {
                    fact.violations.push(violation);
                }
            }
        }
        Ok(fact)
    }
}

impl Contracts {
    /// The fact of a run of `runner` that checked nothing yet.
    fn nothing(&self, runner: ProducerId) -> ContractViolations {
        ContractViolations {
            tool: self.id(),
            runner,
            declared: false,
            runs: Vec::new(),
            tests: 0,
            violations: Vec::new(),
        }
    }
}

/// Whether a manifest of a project root of the language (the repository root when
/// `ProjectInfo` names none), or of one of its members, names the library's package.
fn declared(ctx: &ProduceCtx, library: &Library) -> bool {
    let rule = Rule::Mentions {
        mentions: library.package.to_owned(),
        files: library
            .manifests
            .iter()
            .map(|glob| (*glob).to_owned())
            .collect(),
    };
    let root = canonical(&ctx.ctx.root);
    let roots: Vec<(String, Vec<String>)> = match ctx.upstream.get::<ProjectInfo>() {
        Some(info) if !info.roots.is_empty() => info
            .roots
            .iter()
            .map(|project| (project.path.clone(), project.members.clone()))
            .collect(),
        _ => vec![(".".to_owned(), Vec::new())],
    };
    roots.iter().any(|(dir, members)| {
        let dir = root.join(dir);
        let members: Vec<PathBuf> = members.iter().map(|member| dir.join(member)).collect();
        rule.holds(&Dirs {
            root: &dir,
            members: &members,
        })
    })
}

/// The file a library printed: as given when absolute, else in the first project root that
/// has it.
fn located(dirs: &[PathBuf], file: &str) -> Option<PathBuf> {
    let path = Path::new(file);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    dirs.iter()
        .map(|dir| dir.join(path))
        .find(|path| path.is_file())
}

/// The file a library printed, relative to the repository `root`; `None` outside it.
fn placed(root: &Path, dirs: &[PathBuf], file: &str) -> Option<String> {
    let path = located(dirs, file)?;
    relative(root, root, &path.to_string_lossy())
}

/// Whether a violation concerns the scope: its contract or its test is in a file in scope, or
/// its contract is outside the repository.
fn in_scope(violation: &ContractViolation, test_file: Option<&str>, scope: &Scope) -> bool {
    let within = |file: &str| scope.files.iter().any(|scoped| scoped == file);
    scope.files.is_empty()
        || violation
            .place
            .as_ref()
            .is_none_or(|place| within(&place.file))
        || test_file.is_some_and(within)
}

#[cfg(test)]
pub(crate) mod tests {
    use cupel_fixtures::conformance::{contains, fact_inventories};
    use serde_json::{Value, json};

    use super::*;

    /// The constructs of the `contract-violations` inventory (ADR-113) that none of `found`,
    /// a library's violations on its capture, exhibits.
    pub(crate) fn missed_constructs(found: &[Found]) -> Vec<String> {
        let inventory = fact_inventories()
            .into_iter()
            .find(|inventory| inventory.fact == FactKind::ContractViolations)
            .expect("conformance/contract-violations.toml");
        let fact = json!({
            "violations": found
                .iter()
                .map(|found| json!({"kind": found.kind, "function": found.function, "message": found.message}))
                .collect::<Vec<_>>(),
        });
        inventory
            .constructs
            .iter()
            .filter(|construct| {
                let items = construct
                    .within
                    .as_ref()
                    .and_then(|field| fact[field.as_str()].as_array().cloned())
                    .unwrap_or_default();
                let expect = Value::Object(construct.expect.clone().unwrap_or_default());
                !items.iter().any(|item| contains(item, &expect))
            })
            .map(|construct| construct.id.clone())
            .collect()
    }

    fn violation(place: Option<&str>) -> ContractViolation {
        ContractViolation {
            kind: ContractKind::Precondition,
            function: Some("share".to_owned()),
            message: "parts > 0".to_owned(),
            place: place.map(|file| ContractPlace {
                file: file.to_owned(),
                line: 4,
            }),
            test: "tests::shares".to_owned(),
        }
    }

    #[test]
    fn a_violation_is_in_scope_by_its_contract_or_its_test_or_when_outside_the_repository() {
        let scope = Scope::of_files(vec!["src/lib.rs".to_owned()]);
        assert!(in_scope(&violation(Some("src/lib.rs")), None, &scope));
        assert!(!in_scope(&violation(Some("src/other.rs")), None, &scope));
        assert!(in_scope(
            &violation(Some("src/other.rs")),
            Some("src/lib.rs"),
            &scope
        ));
        assert!(in_scope(&violation(None), None, &scope));
        assert!(in_scope(
            &violation(Some("src/other.rs")),
            None,
            &Scope::of_files(vec![])
        ));
    }

    #[test]
    fn a_printed_file_is_found_in_the_root_that_has_it_and_placed_in_the_repository() {
        let repo = tempfile::tempdir().unwrap();
        let root = canonical(repo.path());
        fs::create_dir_all(root.join("ledger/src")).unwrap();
        fs::write(root.join("ledger/src/lib.rs"), "").unwrap();
        let dirs = vec![root.clone(), root.join("ledger")];
        assert_eq!(
            placed(&root, &dirs, "src/lib.rs"),
            Some("ledger/src/lib.rs".to_owned())
        );
        let absolute = root.join("ledger/src/lib.rs").display().to_string();
        assert_eq!(
            placed(&root, &dirs, &absolute),
            Some("ledger/src/lib.rs".to_owned())
        );
        assert_eq!(placed(&root, &dirs, "/elsewhere/site.py"), None);
        assert_eq!(placed(&root, &dirs, "src/gone.rs"), None);
    }

    #[test]
    fn a_library_is_declared_by_a_manifest_of_a_root_or_a_member() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path();
        fs::create_dir_all(root.join("crates/books")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/books\"]\n",
        )
        .unwrap();
        let mut ctx = ProduceCtx::new(
            Ctx::new(root.to_path_buf()),
            cupel_core::Writer::new(root, false).unwrap().confined(),
            cupel_core::Cancellation::new(),
        );
        assert!(!declared(&ctx, &rust::RUST_CONTRACTS));
        fs::write(
            root.join("crates/books/Cargo.toml"),
            "[dependencies]\ncontracts = \"0.6\"\n",
        )
        .unwrap();
        let info = ProjectInfo {
            roots: vec![cupel_core::syntax::ProjectRoot {
                path: ".".to_owned(),
                lang: LangId::from("rust"),
                build_files: vec!["Cargo.toml".to_owned()],
                members: vec!["crates/books".to_owned()],
                packages: vec![],
                skip_dirs: vec![],
            }],
        };
        ctx.upstream.insert(cupel_core::Produced::new(info).share());
        assert!(declared(&ctx, &rust::RUST_CONTRACTS));
        fs::write(
            root.join("crates/books/Cargo.toml"),
            "[dependencies]\ncontracts-extra = \"1\"\n",
        )
        .unwrap();
        assert!(!declared(&ctx, &rust::RUST_CONTRACTS));
    }
}
