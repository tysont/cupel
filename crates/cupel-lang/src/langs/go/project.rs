// ABOUTME: Go roots for `ProjectInfo`: `go.work` or `go.mod`, and the package patterns commands cover:
// ABOUTME: every package of the module, or of each module a `go.work` uses (ADR-021, ADR-037).
use cupel_core::syntax::ProjectRoot;

use crate::project::{Candidate, Ecosystem};

pub const ECOSYSTEM: Ecosystem = Ecosystem {
    manifests: &["go.work", "go.mod"],
    skip_dirs: &["vendor"],
    describe,
};

fn describe(candidate: &Candidate) -> ProjectRoot {
    let used = candidate
        .read("go.work")
        .map(|work| used_modules(&work))
        .unwrap_or_default();
    let packages = if used.is_empty() {
        vec!["./...".to_owned()]
    } else {
        used.iter().map(|module| packages_of(module)).collect()
    };
    candidate.root("go", packages)
}

/// The pattern for every package of the module in `dir` (a `use` directive's directory).
fn packages_of(dir: &str) -> String {
    match dir.trim_start_matches("./").trim_end_matches('/') {
        "" | "." => "./...".to_owned(),
        dir => format!("./{dir}/..."),
    }
}

/// The directories a `go.work` file's `use` directives name, in order: `use ./a` or a
/// parenthesized block of one directory per line. Comments are ignored.
fn used_modules(work: &str) -> Vec<String> {
    let mut modules = Vec::new();
    let mut in_block = false;
    for line in work.lines() {
        let line = line.split("//").next().unwrap_or_default().trim();
        if in_block {
            if line == ")" {
                in_block = false;
            } else if !line.is_empty() {
                modules.push(line.to_owned());
            }
        } else if let Some(rest) = line.strip_prefix("use") {
            match rest.trim() {
                "(" => in_block = true,
                "" => {}
                module => modules.push(module.to_owned()),
            }
        }
    }
    modules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_directives_list_modules_in_order() {
        let work = "go 1.23\n\nuse ./single // one\n\nuse (\n\t./a\n\n\t./b // two\n)\n";
        assert_eq!(used_modules(work), ["./single", "./a", "./b"]);
        assert!(used_modules("go 1.23\n").is_empty());
    }

    #[test]
    fn a_used_directory_becomes_its_package_pattern() {
        assert_eq!(packages_of("./api"), "./api/...");
        assert_eq!(packages_of("api/"), "./api/...");
        assert_eq!(packages_of("."), "./...");
        assert_eq!(packages_of("./"), "./...");
    }
}
