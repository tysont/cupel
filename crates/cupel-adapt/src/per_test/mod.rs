// ABOUTME: The `PerTestMap` producers (ADR-077): what they share (a spec per tool, run once per project root
// ABOUTME: into a report area under `.cupel/`), and the executed lines of one test's parsed coverage report.
//! Per-test coverage producers.
//!
//! Each producer lists or runs a root's tests under a coverage tool so that every report it reads
//! belongs to one test, and maps the report's executed lines to the runner's test IDs. The
//! reports are read by the `CoverageMap` format parsers (`crate::coverage`), and
//! [`executed_lines`] keeps the lines each test ran.

mod go_cover;
mod llvm_cov;
mod pytest_cov;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cupel_core::coverage::CoverageMap;
use cupel_core::{
    Availability, Captured, CostClass, CoveredLines, Ctx, Error, EventSink, FactKind, Langs,
    PerTestMap, Process, ProduceCtx, Producer, ProducerId, Result, Scope, TestCoverage, ToolId,
    ToolProbe, find_program, output_tail,
};

use crate::runner::{ID_SEPARATOR, roots, under};

/// One per-test producer: everything that differs between Rust, Go, and Python.
pub(crate) struct PerTestSpec {
    /// The producer ID, such as `tool.llvm-cov-per-test`.
    pub id: &'static str,
    pub languages: fn() -> Langs,
    /// The `TestResults` producer whose IDs the map uses.
    pub runner: &'static str,
    /// Every program the producer runs; the first is the one `doctor` probes.
    pub programs: &'static [&'static str],
    /// The command that prints the first program's version.
    pub version: &'static [&'static str],
    /// The tests of one root and the lines each executed, relative to that root.
    pub measure: fn(&Site<'_>) -> Result<Vec<TestCoverage>>,
}

/// A per-test producer: `spec` run once per project root of its language.
pub(crate) struct PerTest(pub &'static PerTestSpec);

/// A project root being measured.
pub(crate) struct Site<'a> {
    pub ctx: &'a ProduceCtx,
    /// The root relative to the repository root; `.` for the root itself.
    pub dir: &'a str,
    /// Absolute and canonical.
    pub path: PathBuf,
    /// Package patterns from `ProjectInfo`.
    pub packages: Vec<String>,
    /// A directory under `.cupel/` for this root's reports.
    pub area: PathBuf,
}

impl Producer<PerTestMap> for PerTest {
    fn id(&self) -> ProducerId {
        ProducerId::from(self.0.id)
    }

    fn languages(&self) -> Langs {
        (self.0.languages)()
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Deep
    }

    fn availability(&self, _: &Ctx) -> Availability {
        match self
            .0
            .programs
            .iter()
            .find(|program| find_program(program).is_none())
        {
            Some(missing) => Availability::MissingTool {
                name: ToolId::from(*missing),
                install: None,
            },
            None => Availability::Ready,
        }
    }

    fn tool(&self, _: &Ctx) -> Option<ToolProbe> {
        Some(ToolProbe {
            name: ToolId::from(self.0.programs[0]),
            version: self
                .0
                .version
                .iter()
                .map(|word| (*word).to_owned())
                .collect(),
        })
    }

    fn produce(&self, ctx: &ProduceCtx, _: &Scope, _: &mut dyn EventSink) -> Result<PerTestMap> {
        let mut tests = Vec::new();
        for (index, (dir, packages)) in roots(ctx).into_iter().enumerate() {
            let path = ctx.ctx.root.join(&dir);
            let area = ctx.writer.create_dir(
                &ctx.writer
                    .cache_dir()
                    .join("per-test")
                    .join(self.0.id)
                    .join(index.to_string()),
            )?;
            let site = Site {
                ctx,
                dir: &dir,
                path: path.canonicalize().unwrap_or(path),
                packages,
                area,
            };
            tests.extend(
                (self.0.measure)(&site)?
                    .into_iter()
                    .map(|test| site.placed(test)),
            );
        }
        Ok(PerTestMap::new(ProducerId::from(self.0.runner), tests))
    }
}

impl Site<'_> {
    /// Runs `program` with `args` in the root, with `env` added, under the run's cancellation.
    pub fn run(&self, program: &str, args: &[String], env: &[(&str, &str)]) -> Result<Captured> {
        let mut process = Process::new(program, &self.path).args(args.iter().cloned());
        for (key, value) in env {
            process = process.env(key, value);
        }
        process.run(&self.ctx.cancel)
    }

    /// Runs `program` with `args` like [`run`](Self::run), failing unless it exits 0.
    pub fn succeed(
        &self,
        program: &str,
        args: &[String],
        env: &[(&str, &str)],
    ) -> Result<Captured> {
        let captured = self.run(program, args, env)?;
        if captured.exit_code == Some(0) {
            Ok(captured)
        } else {
            Err(failed(program, &captured))
        }
    }

    /// A report file in the root's area.
    pub fn report(&self, name: &str) -> PathBuf {
        self.area.join(name)
    }

    /// `path`, as a report names it (absolute, or relative to the root), relative to the root;
    /// `None` for a file outside it.
    pub fn file(&self, path: &str) -> Option<String> {
        let path = Path::new(path);
        let relative = if path.is_absolute() {
            path.strip_prefix(&self.path).ok()?
        } else {
            path
        };
        Some(relative.to_string_lossy().replace('\\', "/"))
    }

    /// `test`, measured relative to this root, made relative to the repository root.
    fn placed(&self, test: TestCoverage) -> TestCoverage {
        TestCoverage::new(
            &under(self.dir, &test.id, ID_SEPARATOR),
            test.file.map(|file| under(self.dir, &file, "/")),
            test.covered
                .into_iter()
                .map(|covered| CoveredLines {
                    file: under(self.dir, &covered.file, "/"),
                    lines: covered.lines,
                })
                .collect(),
        )
    }
}

/// The error of `program` that did not finish its work, with the tail of its output.
pub(crate) fn failed(program: &str, captured: &Captured) -> Error {
    Error::Process {
        program: program.to_owned(),
        message: output_tail(&format!("{}\n{}", captured.stdout, captured.stderr))
            .unwrap_or_else(|| format!("exited with {:?}", captured.exit_code)),
    }
}

/// Lines per file as `CoveredLines`, keeping the files `file` maps into the repository.
pub(crate) fn covered(
    lines: BTreeMap<String, Vec<u32>>,
    file: impl Fn(&str) -> Option<String>,
) -> Vec<CoveredLines> {
    lines
        .into_iter()
        .filter_map(|(path, lines)| {
            Some(CoveredLines {
                file: file(&path)?,
                lines,
            })
        })
        .collect()
}

/// The executed lines (hits above zero) per file of one test's coverage, files as `map` names
/// them.
pub(crate) fn executed_lines(map: &CoverageMap) -> BTreeMap<String, Vec<u32>> {
    map.files
        .iter()
        .map(|file| {
            let lines: Vec<u32> = file
                .lines
                .iter()
                .filter(|hits| hits.hits > 0)
                .map(|hits| hits.line)
                .collect();
            (file.file.clone(), lines)
        })
        .filter(|(_, lines)| !lines.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::executed_lines;
    use crate::coverage::{parse_coverprofile, parse_lcov, placed};

    fn lines(files: &std::collections::BTreeMap<String, Vec<u32>>) -> Vec<(String, Vec<u32>)> {
        files
            .iter()
            .map(|(file, lines)| (file.clone(), lines.clone()))
            .collect()
    }

    #[test]
    fn one_tests_lcov_report_yields_the_lines_it_executed() {
        let capture = Capture::of("per-test/cargo-llvm-cov", "adds");
        let map = parse_lcov(&capture.file("report.lcov")).unwrap();
        assert_eq!(
            lines(&executed_lines(&placed(map, &[&capture.ran_in], "."))),
            [("src/lib.rs".to_owned(), vec![4, 5, 6, 18, 19, 20])]
        );
    }

    #[test]
    fn one_tests_coverprofile_yields_the_lines_of_its_executed_blocks() {
        let read = |test: &str| {
            let profile = Capture::of("per-test/go-cover", test).file("coverprofile.out");
            lines(&executed_lines(&parse_coverprofile(&profile).unwrap()))
        };
        assert_eq!(
            read("TestAdd"),
            [("example.com/calc/calc.go".to_owned(), vec![5, 6, 7])]
        );
        assert_eq!(
            read("TestScale"),
            [("example.com/calc/calc.go".to_owned(), vec![10, 11, 12])]
        );
    }
}
