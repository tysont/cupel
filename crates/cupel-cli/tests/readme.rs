// ABOUTME: Keeps the generated sections of the README and docs/guide (install routes, verbs, flags, exit codes, depths, checks,
// ABOUTME: matrix, settings, MCP tools, agent guide) equal to `describe` and `serve --mcp` output; `make readme` rewrites them (ADR-073).

mod support;

use std::path::PathBuf;

use serde_json::Value;
use support::{described, listed_tools};

/// The command that rewrites the generated sections; failures name it.
const UPDATE_COMMAND: &str = "make readme";

/// Each Markdown file with generated sections, relative to the workspace root, and the sections it
/// holds, each once.
const DOCUMENTS: [(&str, &[&str]); 7] = [
    (
        "README.md",
        &[
            "agent-loop",
            "mcp-tools",
            "check-table",
            "depths",
            "exit-codes",
            "stability",
            "targets",
            "install-routes",
        ],
    ),
    ("docs/guide/install.md", &["install-routes"]),
    ("docs/guide/agents.md", &["exit-codes", "agent-guide"]),
    (
        "docs/guide/usage.md",
        &["verbs", "mcp-tools", "global-flags"],
    ),
    ("docs/guide/checks.md", &["check-table"]),
    ("docs/guide/languages.md", &["support-matrix"]),
    ("docs/guide/configuration.md", &["check-settings"]),
];

fn workspace_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn read(relative: &str) -> String {
    std::fs::read_to_string(workspace_path(relative))
        .unwrap_or_else(|error| panic!("{relative} is readable: {error}"))
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

/// The checks each depth adds and how many run at it: a depth runs every check whose cost class
/// is that depth or a cheaper one, in the order `check --depth` lists the depths.
fn depths(manifests: &[Value]) -> String {
    let check = described(&["check"]);
    let depth = check["arguments"]
        .as_array()
        .expect("check's arguments")
        .iter()
        .find(|argument| argument["name"] == "--depth")
        .expect("check takes --depth");
    let names = strings(&depth["values"]);
    for manifest in manifests {
        assert!(
            names.iter().any(|name| manifest["cost"] == name.as_str()),
            "{} costs {}, which is no depth",
            manifest["id"],
            manifest["cost"]
        );
    }
    let mut running = 0;
    let rows: Vec<Vec<String>> = names
        .iter()
        .map(|name| {
            let mut adds: Vec<String> = manifests
                .iter()
                .filter(|manifest| manifest["cost"] == name.as_str())
                .map(|manifest| manifest["id"].as_str().expect("id").to_owned())
                .collect();
            adds.sort();
            running += adds.len();
            vec![format!("`{name}`"), running.to_string(), code_list(&adds)]
        })
        .collect();
    markdown_table(&owned(&["Depth", "Checks run", "Adds"]), &rows)
}

/// How many of each kind of described entity carry each `stability`, as `cupel describe` reports it.
fn stability(cli: &Value, manifests: &[Value]) -> String {
    let count = |items: Vec<&Value>| -> String {
        let mut levels: Vec<String> = items
            .iter()
            .map(|item| item["stability"].as_str().expect("stability").to_owned())
            .collect();
        levels.sort();
        let mut counted: Vec<(String, usize)> = Vec::new();
        for level in levels {
            match counted.last_mut() {
                Some((last, n)) if *last == level => *n += 1,
                _ => counted.push((level, 1)),
            }
        }
        counted
            .iter()
            .map(|(level, n)| format!("{n} `{level}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let all = |key: &str| -> Vec<&Value> { cli[key].as_array().expect(key).iter().collect() };
    let rows = vec![
        vec!["The CLI".to_owned(), count(vec![cli])],
        vec!["Verbs".to_owned(), count(all("verbs"))],
        vec!["Global flags".to_owned(), count(all("global_flags"))],
        vec!["Exit codes".to_owned(), count(all("exit_codes"))],
        vec!["Checks".to_owned(), count(manifests.iter().collect())],
        vec!["Schemas".to_owned(), count(all("schemas"))],
        vec!["Install routes".to_owned(), count(all("install_routes"))],
    ];
    markdown_table(&owned(&["Described", "Stability"]), &rows)
}

/// One row per release target in `dist/targets.toml`, the file the release workflow and every
/// installer follow (ADR-125).
fn targets() -> String {
    let document: toml_edit::DocumentMut = read("dist/targets.toml")
        .parse()
        .expect("dist/targets.toml is TOML");
    let archive = document["archive"].as_str().expect("an archive pattern");
    let rows: Vec<Vec<String>> = document["target"]
        .as_array_of_tables()
        .expect("[[target]] tables")
        .iter()
        .map(|target| {
            let field = |key: &str| target[key].as_str().expect(key).to_owned();
            vec![
                system(&field("os")).to_owned(),
                field("arch"),
                format!("`{}`", field("triple")),
                format!(
                    "`{}`",
                    archive
                        .replace("{target}", &field("triple"))
                        .replace("{format}", &field("format"))
                ),
            ]
        })
        .collect();
    markdown_table(&owned(&["System", "Arch", "Target", "Archive"]), &rows)
}

/// The start of the guide `cupel describe --agent` prints, through its loop, fenced.
fn agent_loop() -> String {
    let guide = agent_guide();
    let end = guide
        .find("## Exit codes")
        .expect("the guide has an exit-code section after its loop");
    format!("{}…\n```\n", &guide[..end])
}

/// Each generated section's name and body, rendered from the binary's `describe` output and
/// the tools its MCP server lists.
fn generated() -> Vec<(&'static str, String)> {
    let cli = described(&[]);
    let matrix = described(&["matrix"]);
    let manifests = manifests(&matrix);
    vec![
        ("install-routes", install_routes(&cli)),
        ("verbs", verbs(&cli)),
        ("global-flags", global_flags(&cli)),
        ("agent-guide", agent_guide()),
        ("agent-loop", agent_loop()),
        ("exit-codes", exit_codes(&cli)),
        ("depths", depths(&manifests)),
        ("stability", stability(&cli, &manifests)),
        ("targets", targets()),
        ("check-table", check_table(&matrix, &manifests)),
        ("support-matrix", support_matrix(&matrix)),
        ("check-settings", check_settings(&manifests)),
        ("mcp-tools", mcp_tools(&listed_tools())),
    ]
}

/// The names of the generated sections `text` marks, in order.
fn marked(text: &str) -> Vec<&str> {
    text.match_indices("<!-- generated:")
        .map(|(at, prefix)| {
            let name = &text[at + prefix.len()..];
            &name[..name.find(" -->").expect("a closed marker")]
        })
        .collect()
}

/// `text` with the body between each named section's `<!-- generated:NAME -->` and
/// `<!-- end:NAME -->` markers replaced by its rendered body.
fn with_sections(file: &str, text: &str, names: &[&str], sections: &[(&str, String)]) -> String {
    names.iter().fold(text.to_owned(), |text, name| {
        let body = &sections
            .iter()
            .find(|(section, _)| section == name)
            .unwrap_or_else(|| panic!("no generated section is named {name}"))
            .1;
        let begin = format!("<!-- generated:{name} -->\n");
        let end = format!("<!-- end:{name} -->");
        let start = text
            .find(&begin)
            .unwrap_or_else(|| panic!("{file} has no `{}` marker", begin.trim_end()))
            + begin.len();
        let stop = start
            + text[start..]
                .find(&end)
                .unwrap_or_else(|| panic!("{file} has no `{end}` marker"));
        format!("{}{body}{}", &text[..start], &text[stop..])
    })
}

#[test]
fn the_generated_sections_match_describe() {
    let sections = generated();
    let update = std::env::var("UPDATE_README").as_deref() == Ok("1");
    let mut stale = Vec::new();
    for (file, names) in DOCUMENTS {
        let text = read(file);
        let mut found = marked(&text);
        found.sort_unstable();
        let mut listed = names.to_vec();
        listed.sort_unstable();
        assert_eq!(
            found, listed,
            "{file} marks exactly the sections listed for it"
        );
        let expected = with_sections(file, &text, names, &sections);
        if update {
            std::fs::write(workspace_path(file), &expected)
                .unwrap_or_else(|error| panic!("{file} is writable: {error}"));
        } else if text != expected {
            stale.push(file);
        }
    }
    assert!(
        stale.is_empty(),
        "the generated sections of {stale:?} are stale; run `{UPDATE_COMMAND}` and review the diff"
    );
}

/// The Markdown files the README and its guide are made of.
fn documents() -> Vec<String> {
    let guide = workspace_path("docs/guide");
    let mut files: Vec<String> = std::fs::read_dir(&guide)
        .expect("docs/guide is readable")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .map(|path| {
            let name = path.file_name().expect("a file name");
            format!("docs/guide/{}", name.to_str().expect("a UTF-8 name"))
        })
        .collect();
    files.sort();
    files.insert(0, "README.md".to_owned());
    files
}

/// The bodies of `text`'s fenced blocks whose info string is `language`.
fn fenced<'a>(text: &'a str, language: &str) -> Vec<&'a str> {
    let open = format!("```{language}\n");
    text.match_indices(&open)
        .map(|(at, _)| {
            let body = &text[at + open.len()..];
            &body[..body.find("```").expect("a closed fence")]
        })
        .collect()
}

/// Every JSON block is one JSON value or, for JSON Lines such as a recorded trace, several.
#[test]
fn every_json_block_in_the_readme_and_its_guide_parses() {
    for file in documents() {
        for block in fenced(&read(&file), "json") {
            for value in serde_json::Deserializer::from_str(block).into_iter::<Value>() {
                if let Err(error) = value {
                    panic!("a JSON block in {file} does not parse ({error}):\n{block}");
                }
            }
        }
    }
}

/// The README's Claude Code settings: the JSON block that configures a `Stop` hook.
fn stop_hook_command() -> String {
    let readme = read("README.md");
    let settings: Vec<Value> = fenced(&readme, "json")
        .into_iter()
        .filter_map(|block| serde_json::from_str::<Value>(block).ok())
        .filter(|settings| !settings["hooks"]["Stop"].is_null())
        .collect();
    assert_eq!(settings.len(), 1, "the README configures one Stop hook");
    let hook = &settings[0]["hooks"]["Stop"][0]["hooks"][0];
    assert_eq!(hook["type"], "command");
    hook["command"].as_str().expect("a command").to_owned()
}

/// The README's Stop hook exits 2, which keeps Claude Code working and hands it stderr, exactly
/// when `cupel check` fails its gate (exit 1), with what `cupel` printed on stderr; on any other
/// exit it lets Claude stop.
#[cfg(unix)]
#[test]
fn the_readme_stop_hook_blocks_exactly_on_a_failed_gate() {
    use std::os::unix::fs::PermissionsExt;

    let command = stop_hook_command();
    let bin = tempfile::tempdir().expect("temporary directory");
    let cupel = bin.path().join("cupel");
    std::fs::write(
        &cupel,
        "#!/bin/sh\necho \"cupel $*\"\nexit \"$CUPEL_STUB_EXIT\"\n",
    )
    .expect("the stub is writable");
    std::fs::set_permissions(&cupel, std::fs::Permissions::from_mode(0o755))
        .expect("the stub is executable");
    let path = format!("{}:/usr/bin:/bin", bin.path().display());
    for code in 0..=6 {
        let output = std::process::Command::new("sh")
            .args(["-c", &command])
            .env("PATH", &path)
            .env("CUPEL_STUB_EXIT", code.to_string())
            .output()
            .expect("sh runs");
        let expected = if code == 1 { 2 } else { 0 };
        assert_eq!(output.status.code(), Some(expected), "cupel exit {code}");
        assert!(
            output.stdout.is_empty(),
            "the hook prints nothing on stdout"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "cupel check --changed --gate --format text\n",
            "the hook hands cupel's report to Claude on stderr"
        );
    }
}
