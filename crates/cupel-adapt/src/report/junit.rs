// ABOUTME: Parses JUnit XML, as pytest, `cargo nextest`, Maven Surefire, Gradle, and `clojure.test.junit` write it:
// ABOUTME: every `<testcase>` with its class, name, file, time, outcome, and the output of a failure.
use cupel_core::{TestOutcome, TestResult, output_tail};

use super::milliseconds;

/// One `<testcase>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Case {
    pub classname: String,
    pub name: String,
    /// The `file` attribute (pytest's `xunit1` family), as written.
    pub file: Option<String>,
    pub duration_ms: Option<u64>,
    pub outcome: TestOutcome,
    /// For failures and errors: the tail of the message, the body, and the captured output.
    pub output: Option<String>,
}

/// Every test case in `xml`, in document order; `None` when `xml` is not a `JUnit` report.
pub fn parse(xml: &str) -> Option<Vec<Case>> {
    let document = roxmltree::Document::parse(xml).ok()?;
    let root = document.root_element();
    if !matches!(root.tag_name().name(), "testsuites" | "testsuite") {
        return None;
    }
    Some(
        root.descendants()
            .filter(|node| node.has_tag_name("testcase"))
            .map(case)
            .collect(),
    )
}

fn case(node: roxmltree::Node<'_, '_>) -> Case {
    let child = |name: &str| node.children().find(|child| child.has_tag_name(name));
    let (outcome, reason) = if let Some(failure) = child("failure") {
        (TestOutcome::Failed, Some(failure))
    } else if let Some(error) = child("error") {
        (TestOutcome::Errored, Some(error))
    } else if child("skipped").is_some() {
        (TestOutcome::Skipped, None)
    } else {
        (TestOutcome::Passed, None)
    };
    let output = reason.and_then(|reason| {
        let mut text = vec![reason.text().unwrap_or_default().to_owned()];
        if text[0].trim().is_empty() {
            reason
                .attribute("message")
                .unwrap_or_default()
                .clone_into(&mut text[0]);
        }
        text.extend(
            node.children()
                .filter(|child| {
                    child.has_tag_name("system-out") || child.has_tag_name("system-err")
                })
                .filter_map(|child| child.text())
                .map(str::to_owned),
        );
        output_tail(&text.join("\n"))
    });
    Case {
        classname: node.attribute("classname").unwrap_or_default().to_owned(),
        name: node.attribute("name").unwrap_or_default().to_owned(),
        file: node.attribute("file").map(str::to_owned),
        duration_ms: node
            .attribute("time")
            .and_then(|time| time.trim().parse::<f64>().ok())
            .and_then(|seconds| milliseconds(seconds * 1000.0)),
        outcome,
        output,
    }
}

/// The tests in a `cargo nextest` `JUnit` report, with IDs nextest's own `<binary-id> <test path>`.
pub fn nextest(xml: &str) -> Option<Vec<TestResult>> {
    Some(
        parse(xml)?
            .into_iter()
            .map(|case| {
                let id = format!("{} {}", case.classname, case.name);
                case.into_test(id)
            })
            .collect(),
    )
}

/// The tests in a pytest `JUnit` report (`xunit1`, which names files), with IDs pytest's node IDs
/// `<file>::<class>::<name>`.
pub fn pytest(xml: &str) -> Option<Vec<TestResult>> {
    Some(
        parse(xml)?
            .into_iter()
            .map(|case| {
                let id = node_id(&case.classname, &case.name, case.file.as_deref());
                case.into_test(id)
            })
            .collect(),
    )
}

/// The tests in a Maven Surefire or Gradle `JUnit` report (one test class per file), with IDs
/// `<class>#<method>`, the form Surefire's `-Dtest` takes; Gradle's `()` after a method name is
/// dropped. Files are left for the runner to find.
pub fn java(xml: &str) -> Option<Vec<TestResult>> {
    Some(
        parse(xml)?
            .into_iter()
            .map(|case| {
                let method = case.name.strip_suffix("()").unwrap_or(&case.name);
                let id = format!("{}#{method}", case.classname);
                case.into_test(id)
            })
            .collect(),
    )
}

/// The tests in a `clojure.test.junit` report, with IDs the tests' vars, `<namespace>/<name>`.
pub fn clojure(xml: &str) -> Option<Vec<TestResult>> {
    Some(
        parse(xml)?
            .into_iter()
            .map(|case| {
                let id = format!("{}/{}", case.classname, case.name);
                case.into_test(id)
            })
            .collect(),
    )
}

impl Case {
    fn into_test(self, id: String) -> TestResult {
        TestResult {
            id,
            file: self.file,
            name: self.name,
            outcome: self.outcome,
            duration_ms: self.duration_ms,
            output: self.output,
        }
    }
}

/// pytest's node ID: the file, then the classes `classname` names below the file's module, then
/// the test name.
fn node_id(classname: &str, name: &str, file: Option<&str>) -> String {
    let Some(file) = file else {
        return format!("{classname}::{name}");
    };
    let module = file.trim_end_matches(".py").replace('/', ".");
    let classes = classname
        .strip_prefix(&module)
        .map(|rest| rest.trim_start_matches('.'))
        .unwrap_or_default();
    std::iter::once(file)
        .chain(classes.split('.').filter(|class| !class.is_empty()))
        .chain(std::iter::once(name))
        .collect::<Vec<_>>()
        .join("::")
}
