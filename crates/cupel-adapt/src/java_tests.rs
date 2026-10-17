// ABOUTME: What the Maven and Gradle runners share (ADR-108): the classes and methods a selection names, and the
// ABOUTME: JUnit reports one run wrote, one per test class, with each test's source file found below the root.
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::TestResult;

use crate::report::junit;
use crate::runner::{Ran, Selected};

/// The test classes a selection names, in order, each with the methods named of it; a class
/// with no methods runs whole. Test IDs are `<class>#<method>`; a test file names the class its
/// path below a `java` source directory spells (`src/test/java/a/BTest.java` is `a.BTest`).
pub(crate) fn selected_classes(selected: &Selected<'_>) -> Vec<(String, Vec<String>)> {
    let mut classes: Vec<(String, Vec<String>)> = Vec::new();
    let mut add = |class: &str, method: Option<&str>| {
        let index = classes
            .iter()
            .position(|(name, _)| name == class)
            .unwrap_or_else(|| {
                classes.push((class.to_owned(), Vec::new()));
                classes.len() - 1
            });
        if let Some(method) = method {
            classes[index].1.push(method.to_owned());
        }
    };
    match selected {
        Selected::Tests(ids) => {
            for id in *ids {
                match id.split_once('#') {
                    Some((class, method)) => add(class, Some(method)),
                    None => add(id, None),
                }
            }
        }
        Selected::Files(files) => {
            for class in files.iter().filter_map(|file| class_of(file)) {
                add(&class, None);
            }
        }
    }
    classes
}

/// Surefire's `-Dtest` value for a selection: `<class>#<method><sep><method>` per class, classes
/// separated by commas.
#[allow(
    clippy::unnecessary_wraps,
    reason = "every runner narrows through `RunnerSpec::select`; Surefire narrows to anything"
)]
pub(crate) fn class_selection(selected: &Selected<'_>, separator: char) -> Option<String> {
    let entries: Vec<String> = selected_classes(selected)
        .into_iter()
        .map(|(class, methods)| {
            if methods.is_empty() {
                class
            } else {
                format!("{class}#{}", methods.join(&separator.to_string()))
            }
        })
        .collect();
    Some(entries.join(","))
}

/// The class a Java test file declares by its path: the path below its last `java` directory,
/// without `.java`, dotted.
fn class_of(file: &str) -> Option<String> {
    let below = file
        .rsplit_once("/java/")
        .map_or(file, |(_, below)| below)
        .strip_suffix(".java")?;
    Some(below.replace('/', "."))
}

/// The tests in every `TEST-*.xml` below `dirs`, in path order, each with its source file when
/// the root or one of its subprojects has it. A run that wrote no report read nothing, unless
/// it succeeded: a project without tests writes none.
pub(crate) fn read_reports(ran: &Ran<'_>, dirs: &[PathBuf]) -> Option<Vec<TestResult>> {
    let mut reports = Vec::new();
    for dir in dirs {
        collect(dir, &mut reports);
    }
    reports.sort();
    if reports.is_empty() {
        return (ran.captured.exit_code == Some(0)).then(Vec::new);
    }
    let mut tests = Vec::new();
    for report in reports {
        let text = fs::read_to_string(&report).ok()?;
        tests.extend(junit::java(&text)?);
    }
    for test in &mut tests {
        let class = test.id.split('#').next().unwrap_or_default();
        test.file = source_of(&ran.root.dir, class);
    }
    Some(tests)
}

/// Every `TEST-*.xml` file at or below `dir`.
fn collect(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
        } else if entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("TEST-") && name.ends_with(".xml"))
        {
            found.push(path);
        }
    }
}

/// The test source of `class` (its outermost class, for a nested one) in the root's
/// `src/test/java`, or a subproject's one level down, relative to the root.
fn source_of(root: &Path, class: &str) -> Option<String> {
    let outer = class.split('$').next().unwrap_or(class);
    let relative = format!("src/test/java/{}.java", outer.replace('.', "/"));
    if root.join(&relative).is_file() {
        return Some(relative);
    }
    let mut members: Vec<String> = fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| entry.file_name().to_str().map(str::to_owned))
        .collect();
    members.sort();
    members
        .into_iter()
        .map(|member| format!("{member}/{relative}"))
        .find(|candidate| root.join(candidate).is_file())
}

#[cfg(test)]
mod tests {
    use super::{class_selection, selected_classes};
    use crate::runner::Selected;

    #[test]
    fn tests_group_by_class_and_files_name_their_classes() {
        let ids = [
            "a.BTest#one".to_owned(),
            "c.DTest#two".to_owned(),
            "a.BTest#three".to_owned(),
        ];
        assert_eq!(
            class_selection(&Selected::Tests(&ids), '+').as_deref(),
            Some("a.BTest#one+three,c.DTest#two")
        );
        let files = [
            "src/test/java/a/BTest.java".to_owned(),
            "shapes/src/test/java/s/CircleTest.java".to_owned(),
            "README.md".to_owned(),
        ];
        assert_eq!(
            selected_classes(&Selected::Files(&files)),
            [
                ("a.BTest".to_owned(), Vec::new()),
                ("s.CircleTest".to_owned(), Vec::new())
            ]
        );
    }
}
