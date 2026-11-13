// ABOUTME: The Go modules of a project root: its `go.mod`'s module and each module its `go.work` uses, which map a
// ABOUTME: coverage profile's import paths (`example.com/a/fee.go`) to files relative to the root and back.
use std::path::Path;

/// Each module of a root: its import path and its directory relative to the root (empty for the
/// root itself).
pub(crate) struct Modules(Vec<(String, String)>);

impl Modules {
    /// The module of the `go.mod` in `root`, then those of each directory its `go.work` uses.
    pub fn of(root: &Path) -> Self {
        let work = std::fs::read_to_string(root.join("go.work")).unwrap_or_default();
        let modules = std::iter::once(String::new())
            .chain(used(&work))
            .filter_map(|dir| Some((module(&root.join(&dir))?, dir)))
            .collect();
        Self(modules)
    }

    /// The file, relative to the root, that the import path `path` names; `None` for a file of
    /// no module of the root. The longest module path that holds it wins.
    pub fn file(&self, path: &str) -> Option<String> {
        let (dir, rest) = self
            .0
            .iter()
            .filter_map(|(module, dir)| Some((dir, path.strip_prefix(module)?.strip_prefix('/')?)))
            .min_by_key(|(_, rest)| rest.len())?;
        Some(joined(dir, rest))
    }

    /// The import path of the package in `dir`, relative to the root (empty for the root); the
    /// deepest module directory that holds it wins.
    pub fn package(&self, dir: &str) -> Option<String> {
        let (module, rest) = self
            .0
            .iter()
            .filter_map(|(module, at)| Some((module, below(dir, at)?)))
            .min_by_key(|(_, rest)| rest.len())?;
        Some(joined(module, rest))
    }
}

/// `rest` below `dir`, `/`-joined; `rest` alone when `dir` is empty, `dir` when `rest` is.
fn joined(dir: &str, rest: &str) -> String {
    match (dir.is_empty(), rest.is_empty()) {
        (true, _) => rest.to_owned(),
        (false, true) => dir.to_owned(),
        (false, false) => format!("{dir}/{rest}"),
    }
}

/// `dir` relative to `at` (both relative to the root, empty for the root); `None` outside it.
fn below<'a>(dir: &'a str, at: &str) -> Option<&'a str> {
    if at.is_empty() {
        return Some(dir);
    }
    let rest = dir.strip_prefix(at)?;
    if rest.is_empty() {
        Some(rest)
    } else {
        rest.strip_prefix('/')
    }
}

/// The `module` path the `go.mod` in `dir` declares.
fn module(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("go.mod")).ok()?;
    text.lines()
        .find_map(|line| line.trim().strip_prefix("module "))
        .map(|module| module.trim().trim_matches('"').to_owned())
}

/// The directories a `go.work` file's `use` directives name, relative to its directory and
/// without `./` (empty for its own): `use ./a`, or a parenthesized block of one per line.
fn used(work: &str) -> Vec<String> {
    let mut dirs = Vec::new();
    let mut block = false;
    for line in work.lines() {
        let line = line.split("//").next().unwrap_or_default().trim();
        if block {
            block = line != ")";
            if block && !line.is_empty() {
                dirs.push(normalized(line));
            }
        } else if let Some(rest) = line
            .strip_prefix("use")
            .filter(|rest| rest.starts_with([' ', '\t', '(']))
        {
            match rest.trim() {
                "(" => block = true,
                dir => dirs.push(normalized(dir)),
            }
        }
    }
    dirs
}

/// A `use` directive's directory, unquoted, without `./` or a trailing `/`; empty for `.`.
fn normalized(dir: &str) -> String {
    let dir = dir
        .trim_matches('"')
        .trim_start_matches("./")
        .trim_end_matches('/');
    if dir == "." { "" } else { dir }.to_owned()
}

#[cfg(test)]
mod tests {
    use super::{Modules, used};

    #[test]
    fn a_go_works_use_directives_name_its_module_directories() {
        let work = "go 1.21\n\nuse ./single // one\n\nuse (\n\t./a\n\n\t\"./b/\" // two\n\t.\n)\n";
        assert_eq!(used(work), ["single", "a", "b", ""]);
    }

    #[test]
    fn an_import_path_is_the_file_of_the_module_that_holds_it_and_back() {
        let modules = Modules(vec![
            ("example.com/shop".to_owned(), String::new()),
            ("example.com/shop/fees".to_owned(), "fees".to_owned()),
            ("example.com/a".to_owned(), "libs/a".to_owned()),
        ]);
        assert_eq!(
            [
                "example.com/shop/main.go",
                "example.com/shop/fees/fee.go",
                "example.com/a/x/a.go",
                "example.com/other/o.go",
            ]
            .map(|path| modules.file(path)),
            [
                Some("main.go".to_owned()),
                Some("fees/fee.go".to_owned()),
                Some("libs/a/x/a.go".to_owned()),
                None,
            ]
        );
        assert_eq!(
            ["", "cmd", "fees", "libs/a/x"].map(|dir| modules.package(dir)),
            [
                Some("example.com/shop".to_owned()),
                Some("example.com/shop/cmd".to_owned()),
                Some("example.com/shop/fees".to_owned()),
                Some("example.com/a/x".to_owned()),
            ]
        );
    }
}
