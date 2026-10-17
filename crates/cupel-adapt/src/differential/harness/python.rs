// ABOUTME: The Python harness (ADR-093 §6): `python3 -B` runs a driver that imports the entry's module from its
// ABOUTME: import root and answers each input with the call's canonical JSON, or `repr`, or the exception raised.
use super::{Call, Prepared, Problem, Site, write};
use cupel_core::Process;

/// Imports the module, then answers each input line; writes nothing in the tree (`-B`).
const DRIVER: &str = r#"import importlib
import json
import os
import sys

root, module, name = sys.argv[1:4]
sys.path.insert(0, root)
function = getattr(importlib.import_module(module), name)


def show(value):
    try:
        return json.dumps(value, sort_keys=True, allow_nan=False)
    except (TypeError, ValueError):
        return repr(value)


def answer(args):
    try:
        return {"returned": show(function(*args))}
    except Exception as error:
        return {"raised": f"{type(error).__name__}: {error}"}


with open(os.environ["CUPEL_DIFFERENTIAL_IN"]) as inputs, open(
    os.environ["CUPEL_DIFFERENTIAL_OUT"], "a"
) as answers:
    answers.write('{"ready":true}\n')
    answers.flush()
    for line in inputs:
        if line.strip():
            answers.write(json.dumps(answer(json.loads(line))) + "\n")
            answers.flush()
"#;

pub(super) fn prepare(call: &Call<'_>, site: &Site<'_>) -> Result<Prepared, Problem> {
    let driver = write(site, "driver.py", DRIVER)?;
    let (root, module) = import_root(&call.entry.file, &call.resolved.namespace);
    let process = Process::new("python3", site.tree).args([
        "-B".to_owned(),
        driver.to_string_lossy().into_owned(),
        site.tree.join(root).to_string_lossy().into_owned(),
        module,
        call.entry.function.clone(),
    ]);
    Ok(Prepared::new(process, site.work.clone()))
}

/// The directory to put on `sys.path` and the module to import for `file`, whose functions
/// live in the module `namespace`: the file's path less the module's, else the file's own
/// directory and name.
fn import_root(file: &str, namespace: &str) -> (String, String) {
    let stem = file.strip_suffix(".py").unwrap_or(file);
    let stem = stem.strip_suffix("/__init__").unwrap_or(stem);
    let parts = namespace.replace('.', "/");
    let matches = !namespace.is_empty() && (stem == parts || stem.ends_with(&format!("/{parts}")));
    if matches {
        let root = stem[..stem.len() - parts.len()].trim_end_matches('/');
        return (root.to_owned(), namespace.to_owned());
    }
    match stem.rsplit_once('/') {
        Some((root, module)) => (root.to_owned(), module.to_owned()),
        None => (String::new(), stem.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::import_root;

    #[test]
    fn the_import_root_is_the_file_less_its_module_path() {
        assert_eq!(
            import_root("src/shop/pricing.py", "shop.pricing"),
            ("src".to_owned(), "shop.pricing".to_owned())
        );
        assert_eq!(
            import_root("libs/units/src/units/__init__.py", "units"),
            ("libs/units/src".to_owned(), "units".to_owned())
        );
        assert_eq!(
            import_root("quarter.py", "quarter"),
            (String::new(), "quarter".to_owned())
        );
        assert_eq!(
            import_root("tools/run.py", "elsewhere"),
            ("tools".to_owned(), "run".to_owned())
        );
    }
}
