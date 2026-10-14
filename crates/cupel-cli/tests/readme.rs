// ABOUTME: Keeps the README's generated sections (exit codes, check table, support matrix)
// ABOUTME: equal to the binary's `describe` output; `make readme` rewrites them (ADR-073).

mod support;

use std::path::PathBuf;

use serde_json::Value;
use support::run;

/// The command that rewrites the generated sections; failures name it.
const UPDATE_COMMAND: &str = "make readme";

fn readme_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../README.md")
}

/// The `description` of a `cupel describe <args> --format json` that must exit 0.
fn described(args: &[&str]) -> Value {
    let outcome = run(["describe"]
        .into_iter()
        .chain(args.iter().copied())
        .chain(["--format", "json"]));
    assert_eq!(
        outcome.code, 0,
        "cupel describe {args:?}: {}",
        outcome.stdout
    );
    outcome.envelope()["description"].clone()
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|item| item.as_str().expect("a string").to_owned())
        .collect()
}

/// Items as inline code, comma-separated; a dash when there are none.
fn code_list(items: &[String]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("`{item}`")).collect();
    if quoted.is_empty() {
        return "—".to_owned();
    }
    quoted.join(", ")
}

/// Every check's manifest, in the matrix's order.
fn manifests(matrix: &Value) -> Vec<Value> {
    strings(&matrix["checks"])
        .iter()
        .map(|id| described(&[id.as_str()])["manifest"].clone())
        .collect()
}

/// The names of a manifest's metrics whose `per` is (or is not) `scope`.
fn metric_names(manifest: &Value, scope: bool) -> Vec<String> {
    manifest["metrics"]
        .as_array()
        .expect("metrics")
        .iter()
        .filter(|metric| (metric["per"] == "scope") == scope)
        .map(|metric| metric["name"].as_str().expect("metric name").to_owned())
        .collect()
}

fn finding_kinds(manifest: &Value) -> Vec<String> {
    manifest["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .map(|finding| finding["kind"].as_str().expect("kind").to_owned())
        .collect()
}

/// The languages whose matrix cell for `check` is `supported`.
fn supported_languages(matrix: &Value, check: &str) -> Vec<String> {
    matrix["cells"]
        .as_array()
        .expect("cells")
        .iter()
        .filter(|cell| cell["check"] == check && cell["support"]["status"] == "supported")
        .map(|cell| cell["lang"].as_str().expect("lang").to_owned())
        .collect()
}

/// A Markdown table: the header row, the separator, then one line per row.
fn markdown_table(header: &[String], rows: &[Vec<String>]) -> String {
    let line = |cells: &[String]| format!("| {} |\n", cells.join(" | "));
    let separator = vec!["---".to_owned(); header.len()];
    let body: Vec<String> = rows.iter().map(|row| line(row)).collect();
    format!("{}{}{}", line(header), line(&separator), body.concat())
}

fn owned(cells: &[&str]) -> Vec<String> {
    cells.iter().map(|cell| (*cell).to_owned()).collect()
}

fn check_table(matrix: &Value, manifests: &[Value]) -> String {
    let header = owned(&[
        "Check",
        "Tier",
        "Cost",
        "Finding kinds",
        "Gate metrics",
        "Ranked by",
        "Languages",
    ]);
    let rows: Vec<Vec<String>> = manifests
        .iter()
        .map(|manifest| {
            let id = manifest["id"].as_str().expect("id");
            vec![
                format!("`{id}`"),
                manifest["tier"].to_string(),
                manifest["cost"].as_str().expect("cost").to_owned(),
                code_list(&finding_kinds(manifest)),
                code_list(&metric_names(manifest, true)),
                code_list(&metric_names(manifest, false)),
                supported_languages(matrix, id).join(", "),
            ]
        })
        .collect();
    markdown_table(&header, &rows)
}

fn support_matrix(matrix: &Value) -> String {
    let languages = strings(&matrix["languages"]);
    let header: Vec<String> = std::iter::once("Check".to_owned())
        .chain(languages.iter().cloned())
        .collect();
    let rows: Vec<Vec<String>> = strings(&matrix["checks"])
        .iter()
        .map(|check| {
            std::iter::once(format!("`{check}`"))
                .chain(
                    languages
                        .iter()
                        .map(|lang| cell_status(matrix, check, lang)),
                )
                .collect()
        })
        .collect();
    markdown_table(&header, &rows)
}

fn cell_status(matrix: &Value, check: &str, lang: &str) -> String {
    let cell = matrix["cells"]
        .as_array()
        .expect("cells")
        .iter()
        .find(|cell| cell["check"] == check && cell["lang"] == lang)
        .expect("one cell per check and language");
    cell["support"]["status"]
        .as_str()
        .expect("status")
        .to_owned()
}

fn exit_codes(cli: &Value) -> String {
    let rows: Vec<Vec<String>> = cli["exit_codes"]
        .as_array()
        .expect("exit codes")
        .iter()
        .map(|code| {
            vec![
                code["code"].to_string(),
                code["name"].as_str().expect("name").to_owned(),
                code["meaning"].as_str().expect("meaning").to_owned(),
            ]
        })
        .collect();
    markdown_table(&owned(&["Code", "Name", "Meaning"]), &rows)
}

/// Each generated section's name and body, rendered from the binary's `describe` output.
fn generated() -> [(&'static str, String); 3] {
    let matrix = described(&["matrix"]);
    let manifests = manifests(&matrix);
    [
        ("exit-codes", exit_codes(&described(&[]))),
        ("check-table", check_table(&matrix, &manifests)),
        ("support-matrix", support_matrix(&matrix)),
    ]
}

/// `readme` with the body between each section's `<!-- generated:NAME -->` and `<!-- end:NAME -->`
/// markers replaced by its rendered body.
fn with_sections(readme: &str, sections: &[(&str, String)]) -> String {
    sections
        .iter()
        .fold(readme.to_owned(), |text, (name, body)| {
            let begin = format!("<!-- generated:{name} -->\n");
            let end = format!("<!-- end:{name} -->");
            let start = text
                .find(&begin)
                .unwrap_or_else(|| panic!("README.md has no `{}` marker", begin.trim_end()))
                + begin.len();
            let stop = start
                + text[start..]
                    .find(&end)
                    .unwrap_or_else(|| panic!("README.md has no `{end}` marker"));
            format!("{}{body}{}", &text[..start], &text[stop..])
        })
}

/// Finding kinds named in any manifest that `describe finding` does not attribute to their check.
fn undescribed_finding_kinds(manifests: &[Value]) -> Vec<String> {
    manifests
        .iter()
        .flat_map(|manifest| {
            let check = manifest["id"].clone();
            finding_kinds(manifest)
                .into_iter()
                .filter(move |kind| described(&["finding", kind.as_str()])["check"] != check)
        })
        .collect()
}

#[test]
fn the_readme_generated_sections_match_describe() {
    let path = readme_path();
    let readme = std::fs::read_to_string(&path).expect("README.md is readable");
    let expected = with_sections(&readme, &generated());
    if std::env::var("UPDATE_README").as_deref() == Ok("1") {
        std::fs::write(&path, &expected).expect("README.md is writable");
        return;
    }
    assert!(
        readme == expected,
        "README.md's generated sections are stale; run `{UPDATE_COMMAND}` and review the diff"
    );
}

#[test]
fn every_finding_kind_in_the_check_table_is_described() {
    let matrix = described(&["matrix"]);
    assert_eq!(
        undescribed_finding_kinds(&manifests(&matrix)),
        Vec::<String>::new()
    );
}
