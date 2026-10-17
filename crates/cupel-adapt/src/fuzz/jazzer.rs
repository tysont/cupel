// ABOUTME: `tool.jazzer` (ADR-107): a JavaScript project's Jazzer.js fuzz targets (`*.fuzz.js` modules exporting `fuzz`),
// ABOUTME: each run by the `jazzer` CLI with the budget, and the uncaught exception that crashed it read back.
use std::path::Path;

use cupel_core::{Availability, Ctx, FuzzResults, Install, Location, ToolId, ToolProbe};

use super::{Ended, Found, FuzzProducer, Fuzzer, Root, libfuzzer, located, sources};
use crate::runner::{installed, probe};

const ID: &str = "tool.jazzer";

/// What a Jazzer.js fuzz target's file name ends with.
const SUFFIX: &str = ".fuzz.js";

pub struct Jazzer;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<FuzzResults, _>(FuzzProducer(Jazzer)),
});

impl Fuzzer for Jazzer {
    fn id(&self) -> &'static str {
        ID
    }

    fn lang(&self) -> &'static str {
        "javascript"
    }

    fn seeded(&self) -> bool {
        true
    }

    /// Ready with `jazzer` in the root's `node_modules/.bin` or on `PATH`.
    fn availability(&self, ctx: &Ctx) -> Availability {
        if installed("jazzer", true, &ctx.root).is_some() {
            return Availability::Ready;
        }
        Availability::MissingTool {
            name: ToolId::from("jazzer"),
            install: Some(Install::Command(
                "npm install --save-dev @jazzer.js/core".to_owned(),
            )),
        }
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        Some(probe("jazzer", true, &["jazzer", "--version"], &ctx.root))
    }

    /// Every `*.fuzz.js` file under the root, outside `node_modules`.
    fn targets(&self, root: &Root<'_>) -> cupel_core::Result<Option<Vec<Found>>> {
        Ok(Some(
            sources(&root.path, |file, _| file.ends_with(SUFFIX))
                .into_iter()
                .map(|(file, _)| Found {
                    name: file.trim_end_matches(".js").to_owned(),
                    file: Some(file),
                    line: 1,
                })
                .collect(),
        ))
    }

    /// `jazzer <module> <corpora> -- <flags>`: Jazzer.js takes the module without `.js`.
    fn fuzz(&self, root: &Root<'_>, target: &Found, area: &Path) -> cupel_core::Result<Ended> {
        let program = installed("jazzer", true, &root.path).unwrap_or_else(|| "jazzer".to_owned());
        libfuzzer::run(
            libfuzzer::Invocation {
                program: &program,
                dir: &root.path,
                before: vec![target.name.clone()],
                env: Vec::new(),
                separated: true,
            },
            area,
            None,
            &root.budget,
            &root.ctx.cancel,
        )
    }

    fn failure(&self, root: &Root<'_>, output: &str) -> (String, Option<Location>) {
        libfuzzer::reported(exception(&root.ctx.ctx.root, output), output)
    }

    fn reproduce(&self, _: &Root<'_>, target: &Found, input: &str) -> String {
        format!("npx jazzer {} {input}", target.name)
    }
}

/// An uncaught exception's message (`Uncaught Exception: <message>`) and the first frame of its
/// stack in the repository (`at <function> (<file>:<line>:<col>)` or `at <file>:<line>:<col>`).
fn exception(repository: &Path, output: &str) -> Option<(String, Option<Location>)> {
    let mut lines = output.lines();
    let message = lines.by_ref().find_map(|line| {
        line.split_once("Uncaught Exception: ")
            .map(|(_, message)| libfuzzer::masked(message.trim()))
    })?;
    let location = lines
        .take_while(|line| line.trim_start().starts_with("at "))
        .find_map(|line| frame(repository, line));
    Some((message, location))
}

/// The place of one `at …` frame, when it is in the repository.
fn frame(repository: &Path, line: &str) -> Option<Location> {
    let place = line.trim().strip_prefix("at ")?;
    let place = place
        .rsplit_once(" (")
        .map_or(place, |(_, inner)| inner.trim_end_matches(')'));
    let place = place.strip_prefix("file://").unwrap_or(place);
    let mut parts = place.rsplitn(3, ':');
    let col = parts.next()?.parse().ok();
    let line = parts.next()?.parse().ok()?;
    located(repository, parts.next()?, line, col)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use cupel_fixtures::Capture;

    use super::*;

    #[test]
    fn an_uncaught_exception_is_its_message_at_its_first_frame_in_the_repository() {
        let capture = Capture::of("fuzz/jazzer", "crash");
        let elsewhere = tempfile::tempdir().unwrap();
        let (message, location) = exception(elsewhere.path(), &capture.file("output.txt")).unwrap();
        assert_eq!(message, "Error: empty input");
        assert_eq!(location, None);

        let repository = tempfile::tempdir().unwrap();
        fs::write(repository.path().join("first.js"), "").unwrap();
        let output = capture.file("output.txt").replace(
            &capture.ran_in.to_string_lossy().into_owned(),
            &repository.path().to_string_lossy(),
        );
        let (_, location) = exception(repository.path(), &output).unwrap();
        let location = location.unwrap();
        assert_eq!(
            (location.file.as_str(), location.line, location.col),
            ("first.js", 3, Some(11))
        );
    }

    #[test]
    fn a_run_without_an_exception_reports_none() {
        let capture = Capture::of("fuzz/jazzer", "pass");
        assert_eq!(
            exception(&capture.ran_in, &capture.file("output.txt")),
            None
        );
    }
}
