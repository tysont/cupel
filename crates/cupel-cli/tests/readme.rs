// ABOUTME: Keeps the README's generated sections (install routes, verbs, global flags, exit codes, checks, matrix,
// ABOUTME: settings, MCP tools, agent guide) equal to the binary's `describe` and `serve --mcp` output; `make readme` rewrites them (ADR-073).

mod support;

use std::path::PathBuf;

use serde_json::Value;
use support::{described, listed_tools};

/// The command that rewrites the generated sections; failures name it.
const UPDATE_COMMAND: &str = "make readme";

fn readme_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../README.md")
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
        .filter(|cell| cell["check"] == check && status(cell) == "supported")
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

/// One row per setting of every check that has settings (ADR-111); a setting without a default
/// shows a dash.
fn check_settings(manifests: &[Value]) -> String {
    let header = owned(&["Check", "Setting", "Default", "What it sets"]);
    let rows: Vec<Vec<String>> = manifests
        .iter()
        .flat_map(|manifest| {
            let id = manifest["id"].as_str().expect("id");
            // A manifest without settings leaves the field out (ADR-111).
            manifest["settings"]
                .as_array()
                .into_iter()
                .flatten()
                .map(move |setting| {
                    let default = match &setting["default"] {
                        Value::Null => "—".to_owned(),
                        value => format!("`{value}`"),
                    };
                    vec![
                        format!("`{id}`"),
                        format!("`{}`", setting["name"].as_str().expect("setting name")),
                        default,
                        setting["summary"]
                            .as_str()
                            .expect("setting summary")
                            .to_owned(),
                    ]
                })
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
    status(cell)
}

/// A cell's support as the README states it: `unavailable` (a tool missing on the machine that
/// generated it) is `supported` wherever the tool is installed, so the README does not depend on
/// the machine.
fn status(cell: &Value) -> String {
    match cell["support"]["status"].as_str().expect("status") {
        "unavailable" => "supported".to_owned(),
        status => status.to_owned(),
    }
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

/// One row per MCP tool: its name, the command it runs, and its description.
fn mcp_tools(tools: &[Value]) -> String {
    let rows: Vec<Vec<String>> = tools
        .iter()
        .map(|tool| {
            let text = |key: &str| tool[key].as_str().expect("a tool field").to_owned();
            vec![
                format!("`{}`", text("name")),
                format!("`{}`", text("title")),
                text("description"),
            ]
        })
        .collect();
    markdown_table(&owned(&["Tool", "Runs", "What it does"]), &rows)
}

/// One row per verb: its name and summary.
fn verbs(cli: &Value) -> String {
    let rows: Vec<Vec<String>> = cli["verbs"]
        .as_array()
        .expect("verbs")
        .iter()
        .map(|verb| {
            vec![
                format!("`cupel {}`", verb["name"].as_str().expect("name")),
                verb["summary"].as_str().expect("summary").to_owned(),
            ]
        })
        .collect();
    markdown_table(&owned(&["Verb", "Does"]), &rows)
}

/// A table cell's text with its pipes escaped.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// One row per global flag: how it is spelled with its value, its default, the variable that
/// also sets it, and what it does.
fn global_flags(cli: &Value) -> String {
    let rows: Vec<Vec<String>> = cli["global_flags"]
        .as_array()
        .expect("global flags")
        .iter()
        .map(|flag| {
            let name = flag["name"].as_str().expect("name");
            let values = strings(&flag["values"]);
            let spelled = match (values.is_empty(), flag["value_name"].as_str()) {
                (false, _) => format!("{name} {}", values.join("|")),
                (true, Some(value)) => format!("{name} {value}"),
                (true, None) => name.to_owned(),
            };
            let code = |value: &Value| {
                value
                    .as_str()
                    .map_or_else(|| "—".to_owned(), |text| format!("`{text}`"))
            };
            vec![
                cell(&format!("`{spelled}`")),
                code(&flag["default"]),
                code(&flag["env"]),
                cell(flag["summary"].as_str().expect("summary")),
            ]
        })
        .collect();
    markdown_table(&owned(&["Flag", "Default", "Variable", "Meaning"]), &rows)
}

/// The guide `cupel describe --agent` prints, fenced so its headings stay inside the block.
fn agent_guide() -> String {
    let outcome = support::run(["describe", "--agent", "--format", "text"]);
    assert_eq!(outcome.code, 0, "{}", outcome.stdout);
    assert!(!outcome.stdout.contains("```"), "the guide holds no fence");
    format!("```markdown\n{}```\n", outcome.stdout)
}

/// An operating system as Rust spells it (`std::env::consts::OS`), as people write it.
fn system(os: &str) -> &str {
    match os {
        "linux" => "Linux",
        "macos" => "macOS",
        "windows" => "Windows",
        other => other,
    }
}

/// One row per install route (ADR-128): its name, command, platforms, and `now` when it is live
/// or what it waits for.
fn install_routes(cli: &Value) -> String {
    let rows: Vec<Vec<String>> = cli["install_routes"]
        .as_array()
        .expect("install routes")
        .iter()
        .map(|route| {
            let platforms = strings(&route["platforms"]);
            let platforms: Vec<&str> = platforms.iter().map(|os| system(os)).collect();
            vec![
                route["name"].as_str().expect("name").to_owned(),
                cell(&format!(
                    "`{}`",
                    route["command"].as_str().expect("command")
                )),
                platforms.join(", "),
                route["pending"].as_str().unwrap_or("now").to_owned(),
            ]
        })
        .collect();
    markdown_table(
        &owned(&["Route", "Command", "Platforms", "Available"]),
        &rows,
    )
}

/// Each generated section's name and body, rendered from the binary's `describe` output and
/// the tools its MCP server lists.
fn generated() -> [(&'static str, String); 9] {
    let cli = described(&[]);
    let matrix = described(&["matrix"]);
    let manifests = manifests(&matrix);
    [
        ("install-routes", install_routes(&cli)),
        ("verbs", verbs(&cli)),
        ("global-flags", global_flags(&cli)),
        ("agent-guide", agent_guide()),
        ("exit-codes", exit_codes(&cli)),
        ("check-table", check_table(&matrix, &manifests)),
        ("support-matrix", support_matrix(&matrix)),
        ("check-settings", check_settings(&manifests)),
        ("mcp-tools", mcp_tools(&listed_tools())),
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
