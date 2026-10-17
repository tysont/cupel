// ABOUTME: The Go ecosystem of `tool.footprint` (ADR-103): every module `go.mod` requires, direct unless marked
// ABOUTME: `// indirect`; Go modules declare no license outside their LICENSE text, so none is read.
use cupel_core::Dependency;

use super::{Context, Ecosystem, Project, Root};

pub static GO: Ecosystem = Ecosystem {
    langs: &["go"],
    osv: "Go",
    discover,
    parse,
    normalize: str::to_owned,
};

const MANIFEST: &str = "go.mod";

fn discover(root: &Root<'_>) -> Project {
    Project {
        manifests: if root.path.join(MANIFEST).is_file() {
            vec![root.file(MANIFEST)]
        } else {
            Vec::new()
        },
        context: Context::default(),
    }
}

/// The `require` lines of `go.mod`, in a block or one by one.
fn parse(file: &str, text: &str, _: &Context) -> Vec<Dependency> {
    let mut found = Vec::new();
    let mut in_block = false;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        let required = if in_block {
            if line.starts_with(')') {
                in_block = false;
                continue;
            }
            line
        } else if line == "require (" {
            in_block = true;
            continue;
        } else if let Some(rest) = line.strip_prefix("require ") {
            rest.trim()
        } else {
            continue;
        };
        let (spec, comment) = required
            .split_once("//")
            .map_or((required, ""), |(spec, comment)| (spec, comment));
        let mut words = spec.split_whitespace();
        let (Some(name), Some(version)) = (words.next(), words.next()) else {
            continue;
        };
        found.push(Dependency {
            name: name.to_owned(),
            version: Some(version.to_owned()),
            direct: comment.trim() != "indirect",
            license: None,
            file: file.to_owned(),
            line: u32::try_from(index + 1).unwrap_or(u32::MAX),
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requirements_in_blocks_and_lines_with_indirect_marks() {
        let text = "module example.com/ledger\n\ngo 1.22\n\nrequire github.com/google/uuid v1.6.0\n\nrequire (\n\tgithub.com/stretchr/testify v1.9.0\n\tgithub.com/davecgh/go-spew v1.1.1 // indirect\n)\n";
        let found: Vec<_> = parse("go.mod", text, &Context::default())
            .into_iter()
            .map(|d| (d.name, d.version.unwrap(), d.direct, d.line))
            .collect();
        assert_eq!(
            found,
            [
                (
                    "github.com/google/uuid".to_owned(),
                    "v1.6.0".to_owned(),
                    true,
                    5
                ),
                (
                    "github.com/stretchr/testify".to_owned(),
                    "v1.9.0".to_owned(),
                    true,
                    8
                ),
                (
                    "github.com/davecgh/go-spew".to_owned(),
                    "v1.1.1".to_owned(),
                    false,
                    9
                ),
            ]
        );
    }
}
