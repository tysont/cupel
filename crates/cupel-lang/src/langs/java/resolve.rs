// ABOUTME: Java module resolution for `ModuleGraph`: source roots found from package declarations and the
// ABOUTME: `src/<set>/java` convention, each imported type mapped to its file, the JDK and other packages external.
//!
//! Queries find imported names; which file declares a type depends on where the source roots
//! are, which a query cannot see, so this part of the language is hand-written (ADR-108).
//!
//! - **Source roots** are the directories a file's package path starts below: for every Java file
//!   in scope, its directory with the package's path (`ledger/report` for `package
//!   ledger.report;`) removed from its end; and every `src/<set>/java` directory of the
//!   repository, so files outside the scope resolve too.
//! - **A type import** (`import a.b.C;`) names `a/b/C.java` in some source root. A static import
//!   or a nested type (`import a.b.C.m;`) names the file of its outermost type: the longest
//!   prefix of the name that is a file wins.
//! - **An import on demand** (`import a.b.*;`) names every Java file of the package directory
//!   `a/b` in every source root, in path order, or the file of the type `a.b` when it is one.
//! - **The JDK** is every name in `java.`, `javax.`, `jdk.`, `sun.`, `com.sun.`, and the XML and
//!   security packages the JDK ships (`org.w3c.dom`, `org.xml.sax`, `org.ietf.jgss`); it is
//!   `External` with package `jdk`.
//! - **Anything else** is `Unresolved` when a prefix of its package is a package the repository
//!   declares (a directory of Java files in a source root: generated code, a typo), and otherwise
//!   `External` with its package: the name's segments before the first capitalized one.
//! - Edges are grouped by project directory: the nearest directory above the file with a Maven or
//!   Gradle build file (`.` for files outside every project). Java has no relative imports.
use std::collections::{BTreeMap, BTreeSet};

use cupel_core::syntax::{ImportEdge, ImportTarget, ModuleGraph, RootImports};

use crate::imports::{Imports, join};

/// The package the JDK's types are reported as.
const JDK: &str = "jdk";

/// Package prefixes the JDK ships.
const JDK_PREFIXES: [&str; 8] = [
    "java.",
    "javax.",
    "jdk.",
    "sun.",
    "com.sun.",
    "org.w3c.dom",
    "org.xml.sax",
    "org.ietf.jgss",
];

/// Files whose presence makes a directory a project.
const BUILD_FILES: [&str; 5] = [
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
];

/// Directories never searched for source roots: build output and tool state.
const GENERATED: [&str; 4] = ["target", "build", "out", "node_modules"];

/// How deep below the repository root `src/<set>/java` directories are searched for.
const SEARCH_DEPTH: usize = 6;

/// Resolves every import in scope.
pub fn graph(imports: &Imports<'_>) -> ModuleGraph {
    let roots = source_roots(imports);
    let mut grouped: BTreeMap<String, Vec<ImportEdge>> = BTreeMap::new();
    for file in imports.scope() {
        let edges = grouped.entry(project_of(imports, file)).or_default();
        for import in imports.of(file).imports {
            let on_demand = import.names.iter().any(|name| name == "*");
            for target in resolve(imports, &roots, &import.specifier, on_demand) {
                edges.push(ImportEdge {
                    from: file.clone(),
                    specifier: import.specifier.clone(),
                    target,
                    relative: false,
                    line: import.line,
                    edge_kind: import.kind,
                });
            }
        }
    }
    ModuleGraph {
        roots: grouped
            .into_iter()
            .map(|(root, mut edges)| {
                edges.sort_by(|a, b| {
                    (&a.from, a.line, &a.specifier).cmp(&(&b.from, b.line, &b.specifier))
                });
                RootImports { root, edges }
            })
            .collect(),
    }
}

/// What the imported name `name` refers to: the files of its type, or of its package's types
/// when it is imported on demand.
fn resolve(
    imports: &Imports<'_>,
    roots: &BTreeSet<String>,
    name: &str,
    on_demand: bool,
) -> Vec<ImportTarget> {
    let path = name.replace('.', "/");
    if on_demand {
        let files: Vec<ImportTarget> = roots
            .iter()
            .flat_map(|root| imports.files_in(&join(root, &path), ".java"))
            .filter(|file| !is_descriptor(file))
            .map(|path| ImportTarget::File { path })
            .collect();
        if !files.is_empty() {
            return files;
        }
    }
    if let Some(file) = type_file(imports, roots, &path) {
        return vec![ImportTarget::File { path: file }];
    }
    vec![outside(imports, roots, name)]
}

/// The file declaring the outermost type of `path` (`a/b/C/D` for `a.b.C.D`): the longest
/// prefix of it that names a Java file in a source root.
fn type_file(imports: &Imports<'_>, roots: &BTreeSet<String>, path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('/').collect();
    (1..=segments.len()).rev().find_map(|length| {
        let candidate = format!("{}.java", segments[..length].join("/"));
        roots
            .iter()
            .map(|root| join(root, &candidate))
            .find(|file| imports.is_file(file))
    })
}

/// What a name that names no file is: the JDK, a missing part of a package the repository
/// declares, or another library's package.
fn outside(imports: &Imports<'_>, roots: &BTreeSet<String>, name: &str) -> ImportTarget {
    if JDK_PREFIXES.iter().any(|prefix| name.starts_with(prefix)) {
        return ImportTarget::External {
            package: JDK.to_owned(),
        };
    }
    let package: Vec<&str> = name
        .split('.')
        .take_while(|segment| !segment.starts_with(char::is_uppercase) && *segment != "*")
        .collect();
    let declared = (1..=package.len()).any(|length| {
        let dir = package[..length].join("/");
        roots
            .iter()
            .any(|root| !imports.files_in(&join(root, &dir), ".java").is_empty())
    });
    if declared || package.is_empty() {
        ImportTarget::Unresolved
    } else {
        ImportTarget::External {
            package: package.join("."),
        }
    }
}

/// `module-info.java` and `package-info.java` declare no type.
fn is_descriptor(file: &str) -> bool {
    file.ends_with("/module-info.java") || file.ends_with("/package-info.java")
}

/// Every source root: the root each Java file in scope declares through its package, and every
/// `src/<set>/java` directory below the repository root.
fn source_roots(imports: &Imports<'_>) -> BTreeSet<String> {
    let mut roots: BTreeSet<String> = imports
        .scope()
        .iter()
        .filter_map(|file| declared_root(file, &package_of(&imports.read(file)?)?))
        .collect();
    conventional_roots(imports, ".", 0, &mut roots);
    roots
}

/// The directory `file` sits in with the path of `package` removed from its end; `None` when
/// the file is not where its package says (the compiler does not require it).
fn declared_root(file: &str, package: &str) -> Option<String> {
    let dir = file.rsplit_once('/').map_or("", |(dir, _)| dir);
    if package.is_empty() {
        return Some(if dir.is_empty() {
            ".".to_owned()
        } else {
            dir.to_owned()
        });
    }
    let suffix = package.replace('.', "/");
    if dir == suffix {
        return Some(".".to_owned());
    }
    dir.strip_suffix(&format!("/{suffix}")).map(str::to_owned)
}

/// The package a Java source declares (`package a.b;`), empty for the default package; `None`
/// when the text has no type or module declaration to tell (an empty file).
fn package_of(text: &str) -> Option<String> {
    let mut in_comment = false;
    for line in text.lines() {
        let mut line = line.trim();
        if in_comment {
            match line.find("*/") {
                Some(end) => {
                    line = line[end + 2..].trim();
                    in_comment = false;
                }
                None => continue,
            }
        }
        if line.starts_with("/*") && !line.contains("*/") {
            in_comment = true;
            continue;
        }
        if line.is_empty()
            || line.starts_with("//")
            || line.starts_with("/*")
            || line.starts_with('@')
        {
            continue;
        }
        return Some(
            line.strip_prefix("package ")
                .and_then(|rest| rest.split(';').next())
                .map(|name| name.split_whitespace().collect())
                .unwrap_or_default(),
        );
    }
    None
}

/// Adds every `src/<set>/java` directory at or below `dir` (to `SEARCH_DEPTH`) to `roots`,
/// skipping hidden and generated directories.
fn conventional_roots(
    imports: &Imports<'_>,
    dir: &str,
    depth: usize,
    roots: &mut BTreeSet<String>,
) {
    if depth > SEARCH_DEPTH {
        return;
    }
    for child in imports.dirs_in(dir) {
        let name = child.rsplit('/').next().unwrap_or_default();
        if name.starts_with('.') || GENERATED.contains(&name) {
            continue;
        }
        if name == "src" {
            for set in imports.dirs_in(&child) {
                let java = join(&set, "java");
                if imports.is_dir(&java) {
                    roots.insert(java);
                }
            }
        }
        conventional_roots(imports, &child, depth + 1, roots);
    }
}

/// The project directory of `file`: the nearest directory above it with a build file.
fn project_of(imports: &Imports<'_>, file: &str) -> String {
    let mut dir = file.rsplit_once('/').map_or("", |(dir, _)| dir).to_owned();
    loop {
        let at = if dir.is_empty() { "." } else { dir.as_str() };
        if BUILD_FILES
            .iter()
            .any(|build| imports.is_file(&join(at, build)))
        {
            return at.to_owned();
        }
        match dir.rsplit_once('/') {
            Some((parent, _)) => dir = parent.to_owned(),
            None if !dir.is_empty() => dir.clear(),
            None => return ".".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{declared_root, package_of};

    #[test]
    fn a_package_declaration_is_read_past_comments_and_annotations() {
        let text =
            "/*\n * Licensed.\n */\n// a comment\n@Generated\npackage a.b.c ;\n\nclass X {}\n";
        assert_eq!(package_of(text).as_deref(), Some("a.b.c"));
        assert_eq!(package_of("class X {}\n").as_deref(), Some(""));
        assert_eq!(package_of("// nothing\n"), None);
    }

    #[test]
    fn a_source_root_is_the_directory_above_the_package_path() {
        assert_eq!(
            declared_root("src/main/java/a/b/X.java", "a.b").as_deref(),
            Some("src/main/java")
        );
        assert_eq!(declared_root("a/b/X.java", "a.b").as_deref(), Some("."));
        assert_eq!(declared_root("src/X.java", "").as_deref(), Some("src"));
        assert_eq!(declared_root("src/elsewhere/X.java", "a.b"), None);
    }
}
