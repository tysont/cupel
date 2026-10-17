// ABOUTME: The property frameworks the engine budgets (ADR-095 §3): the runners each runs under, whether it takes the
// ABOUTME: seed, the environment, arguments, and files that hand it the seed and case budget, and its report parser.
use std::path::{Path, PathBuf};

use cupel_core::{Error, ProduceCtx, ProducerId, Result, RunOptions};

use super::counterexample;

/// What a property run hands every framework: the run's seed and the case budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    pub cases: u32,
    pub seed: u64,
}

/// One property framework.
pub struct Framework {
    /// As `TestCase::property` and the toolchain table's `property` column name it.
    pub name: &'static str,
    /// The runners that can take its budget; `None` for every runner of its language.
    runners: Option<Runners>,
    /// Whether it takes the run's seed.
    pub seeded: bool,
    /// The shrunk counterexample in a failing test's output.
    pub counterexample: fn(&str) -> Option<String>,
    /// The settings that hand it the budget under `runner`.
    settings: fn(&ProduceCtx, &ProducerId, &Budget) -> Result<RunOptions>,
}

/// The only runners a framework's budget reaches, and the error that names them.
struct Runners {
    ids: &'static [&'static str],
    what: &'static str,
    expected: &'static str,
}

/// Every framework the engine budgets.
pub const FRAMEWORKS: [Framework; 4] = [
    Framework {
        name: "proptest",
        runners: None,
        seeded: true,
        counterexample: counterexample::proptest,
        settings: |_, _, budget| {
            Ok(environment(&[
                ("PROPTEST_CASES", budget.cases.to_string()),
                ("PROPTEST_RNG_SEED", budget.seed.to_string()),
                ("PROPTEST_DISABLE_FAILURE_PERSISTENCE", "1".to_owned()),
            ]))
        },
    },
    Framework {
        name: "quickcheck",
        runners: None,
        seeded: false,
        counterexample: counterexample::quickcheck,
        settings: |_, _, budget| {
            Ok(environment(&[
                ("QUICKCHECK_TESTS", budget.cases.to_string()),
                (
                    "QUICKCHECK_MAX_TESTS",
                    u64::from(budget.cases).saturating_mul(10).to_string(),
                ),
            ]))
        },
    },
    Framework {
        name: "hypothesis",
        runners: Some(Runners {
            ids: &["tool.pytest"],
            what: "runner for hypothesis properties",
            expected: "tool.pytest",
        }),
        seeded: true,
        counterexample: counterexample::hypothesis,
        settings: hypothesis,
    },
    Framework {
        name: "fast-check",
        runners: Some(Runners {
            ids: &["tool.vitest", "tool.jest"],
            what: "runner for fast-check properties",
            expected: "tool.vitest or tool.jest",
        }),
        seeded: true,
        counterexample: counterexample::fast_check,
        settings: fast_check,
    },
];

/// The framework called `name`.
pub fn find(name: &str) -> Option<&'static Framework> {
    FRAMEWORKS.iter().find(|framework| framework.name == name)
}

impl Framework {
    /// The options that hand this framework `budget` under `runner`, writing the files it needs
    /// under `.cupel/cache/property/`; an error when `runner` cannot take them.
    pub fn options(
        &self,
        ctx: &ProduceCtx,
        runner: &ProducerId,
        budget: &Budget,
    ) -> Result<RunOptions> {
        if let Some(runners) = &self.runners
            && !runners.ids.contains(&runner.as_str())
        {
            return Err(Error::InvalidValue {
                what: runners.what,
                value: runner.to_string(),
                expected: runners.expected,
            });
        }
        (self.settings)(ctx, runner, budget)
    }
}

fn environment(variables: &[(&str, String)]) -> RunOptions {
    RunOptions {
        env: variables
            .iter()
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect(),
        ..RunOptions::default()
    }
}

/// The directory the engine writes its files in.
fn area(ctx: &ProduceCtx) -> PathBuf {
    ctx.writer.cache_dir().join("property")
}

/// Writes `contents` to `name` in the engine's directory; where it landed.
fn write(ctx: &ProduceCtx, name: &str, contents: &str) -> Result<PathBuf> {
    ctx.writer.write(&area(ctx).join(name), contents.as_bytes())
}

/// Hypothesis under pytest: a plugin that loads a profile with the budget, the seed as
/// `--hypothesis-seed`, and Hypothesis's own files kept under `.cupel/`.
fn hypothesis(ctx: &ProduceCtx, _: &ProducerId, budget: &Budget) -> Result<RunOptions> {
    let plugin = write(
        ctx,
        "python/cupel_property.py",
        &include_str!("cupel_property.py").replace("{cases}", &budget.cases.to_string()),
    )?;
    let dir = plugin.parent().unwrap_or(Path::new("."));
    let path = std::env::var_os("PYTHONPATH").map_or_else(
        || dir.display().to_string(),
        |existing| format!("{}:{}", dir.display(), existing.to_string_lossy()),
    );
    let storage = ctx.writer.create_dir(&area(ctx).join("hypothesis"))?;
    Ok(environment(&[
        (
            "PYTEST_ADDOPTS",
            format!("-p cupel_property --hypothesis-seed={}", budget.seed),
        ),
        ("PYTHONPATH", path),
        (
            "HYPOTHESIS_STORAGE_DIRECTORY",
            storage.display().to_string(),
        ),
    ]))
}

/// fast-check under vitest or jest: a setup file that configures the budget, appended to the
/// project's setup files by a configuration file passed as `--config`.
fn fast_check(ctx: &ProduceCtx, runner: &ProducerId, budget: &Budget) -> Result<RunOptions> {
    let configure = format!(
        "fc.configureGlobal({{ ...fc.readConfigureGlobal(), numRuns: {}, seed: {} }});\n",
        budget.cases,
        budget.seed % (1 << 31)
    );
    let (setup, config, template) = if runner.as_str() == "tool.jest" {
        (
            write(
                ctx,
                "fast-check.setup.cjs",
                &format!("const fc = require(\"fast-check\");\n\n{configure}"),
            )?,
            "jest.config.cjs",
            include_str!("jest.config.cjs"),
        )
    } else {
        (
            write(
                ctx,
                "fast-check.setup.mjs",
                &format!("import fc from \"fast-check\";\n\n{configure}"),
            )?,
            "vitest.config.mjs",
            include_str!("vitest.config.mjs"),
        )
    };
    let quoted = serde_json::to_string(&setup.display().to_string()).unwrap_or_default();
    let config = write(ctx, config, &template.replace("{setup}", &quoted))?;
    Ok(RunOptions {
        args: vec!["--config".to_owned(), config.display().to_string()],
        ..RunOptions::default()
    })
}
