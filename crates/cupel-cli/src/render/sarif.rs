// ABOUTME: Renders an envelope as a SARIF 2.1.0 log: one rule per finding kind, one result per finding.
// ABOUTME: Whatever SARIF has no place for rides in `properties` (ADR-099); Cupel never offers `fixes`.
use std::fmt::Write as _;
use std::path::Path;

use cupel_core::{ActionKind, Check, CheckId, Envelope, Finding, Location, Severity, Status};
use serde_json::{Map, Value, json};

use crate::catalog::Catalog;

/// Where the log says its schema lives: the OASIS standard's copy.
const SCHEMA: &str =
    "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json";

/// The base every file location is relative to: the repository root.
const SOURCE_ROOT: &str = "SRCROOT";

/// The finding fields a result shows in its own SARIF properties; the rest go in `properties`.
const MAPPED_FINDING_FIELDS: [&str; 6] = [
    "id",
    "severity",
    "title",
    "detail",
    "location",
    "suppressed",
];

/// `envelope` as a pretty SARIF 2.1.0 log ending with a newline. `root`, the repository root,
/// becomes the `SRCROOT` base of every file location and the repository of the git revision.
pub fn render(envelope: &Envelope, root: Option<&Path>) -> String {
    let mut out = serde_json::to_string_pretty(&log(envelope, root)).expect("SARIF serializes");
    out.push('\n');
    out
}

fn log(envelope: &Envelope, root: Option<&Path>) -> Value {
    let catalog = Catalog::builtin();
    let kinds = rule_kinds(envelope, &catalog);
    let rules: Vec<Value> = kinds
        .iter()
        .map(|(kind, check)| rule(kind, check, &catalog))
        .collect();
    let against_baseline = envelope.run.baseline.is_some();
    let results: Vec<Value> = envelope
        .findings
        .iter()
        .map(|finding| {
            let index = kinds
                .iter()
                .position(|(kind, _)| *kind == finding.kind)
                .expect("every finding kind has a rule");
            result(finding, index, against_baseline)
        })
        .collect();
    let mut run = json!({
        "tool": {"driver": {
            "name": "cupel",
            "version": envelope.cupel.version,
            "semanticVersion": envelope.cupel.version,
            "rules": rules,
        }},
        "automationDetails": {"id": automation_id(envelope)},
        "invocations": [invocation(envelope)],
        "results": results,
        "properties": without(envelope, &["run", "findings"]),
    });
    if let Some(root) = root {
        locate(&mut run, envelope, root);
    }
    json!({"$schema": SCHEMA, "version": "2.1.0", "runs": [run]})
}

/// Adds the repository at `root` to `run`: as the base of file locations and, in a git
/// repository, as where the revision lives.
fn locate(run: &mut Value, envelope: &Envelope, root: &Path) {
    let uri = directory_uri(root);
    run["originalUriBaseIds"] = json!({SOURCE_ROOT: {"uri": uri}});
    if let Some(git) = &envelope.run.git {
        let mut revision = json!({
            "repositoryUri": uri,
            "revisionId": git.sha,
            "mappedTo": {"uriBaseId": SOURCE_ROOT},
        });
        if let Some(branch) = &git.branch {
            revision["branch"] = json!(branch);
        }
        run["versionControlProvenance"] = json!([revision]);
    }
}

/// Each rule's kind and the check it belongs to: every kind the checks that ran declare, in
/// their order, then every other kind a finding has, each kind once.
fn rule_kinds(envelope: &Envelope, catalog: &Catalog) -> Vec<(String, CheckId)> {
    let declared = envelope.checks.iter().flat_map(|result| {
        catalog
            .checks()
            .get(&result.id)
            .map(|check| check.manifest().findings.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|spec| (spec.kind, result.id.clone()))
    });
    let found = envelope
        .findings
        .iter()
        .map(|finding| (finding.kind.clone(), finding.check.clone()));
    let mut kinds: Vec<(String, CheckId)> = Vec::new();
    for (kind, check) in declared.chain(found) {
        if !kinds.iter().any(|(known, _)| *known == kind) {
            kinds.push((kind, check));
        }
    }
    kinds
}

/// The rule for `kind` of `check`: described from the catalog when it knows the kind (what
/// `cupel describe finding` prints), and from its name alone when it does not, as for a plugin.
fn rule(kind: &str, check: &CheckId, catalog: &Catalog) -> Value {
    let described = catalog.finding(kind);
    let manifest = catalog.checks().get(check).map(Check::manifest);
    let summary = described.as_ref().map_or_else(
        || format!("A `{kind}` finding of {check}."),
        |described| described.summary.clone(),
    );
    let mut properties = json!({"check": check});
    if let Some(described) = &described {
        properties["action"] = json!(described.action);
        properties["stability"] = json!(described.stability);
    }
    if let Some(manifest) = manifest {
        properties["tier"] = json!(manifest.tier);
        properties["cost"] = json!(manifest.cost);
    }
    let action = described.as_ref().map(|described| described.action);
    let mut rule = json!({
        "id": kind,
        "shortDescription": {"text": summary},
        "help": {"text": help(kind, check, &summary, action)},
        "properties": properties,
    });
    if let Some(manifest) = manifest {
        rule["fullDescription"] = json!({"text": manifest.summary});
    }
    rule
}

/// A rule's help: its summary, what to do, and where `describe` and `explain` say more.
fn help(kind: &str, check: &CheckId, summary: &str, action: Option<ActionKind>) -> String {
    let mut help = format!("{summary}\n\n");
    if let Some(action) = action {
        let _ = write!(help, "Action: {}. ", name_of(&action));
    }
    let _ = write!(
        help,
        "`cupel describe finding {kind}` describes the kind and its evidence, \
         `cupel describe {check}` the check, and `cupel explain <finding-id>` one finding in \
         context."
    );
    help
}

/// The result for `finding`, whose rule is at `index`. `against_baseline` says whether the run
/// compared with a baseline, which is when `new_since_baseline` means anything.
fn result(finding: &Finding, index: usize, against_baseline: bool) -> Value {
    let mut fingerprints = json!({"cupel/findingId": finding.id});
    if let Some(symbol) = &finding.symbol {
        fingerprints["cupel/symbolHash"] = json!(symbol.hash);
    }
    let suppressions = if finding.suppressed {
        json!([{"kind": "external", "status": "accepted"}])
    } else {
        json!([])
    };
    let mut result = json!({
        "ruleId": finding.kind,
        "ruleIndex": index,
        "level": level(finding.severity),
        "message": {"text": message(finding)},
        "partialFingerprints": fingerprints,
        "suppressions": suppressions,
        "fixes": [],
        "properties": without(finding, &MAPPED_FINDING_FIELDS),
    });
    if let Some(location) = location(finding) {
        result["locations"] = json!([location]);
    }
    if against_baseline {
        let state = if finding.new_since_baseline {
            "new"
        } else {
            "unchanged"
        };
        result["baselineState"] = json!(state);
    }
    result
}

fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Fail => "error",
        Severity::Warn => "warning",
        Severity::Info => "note",
    }
}

/// The title, then the detail after a blank line when there is one.
fn message(finding: &Finding) -> String {
    match &finding.detail {
        Some(detail) => format!("{}\n\n{detail}", finding.title),
        None => finding.title.clone(),
    }
}

/// Where the finding is: its file region, and its symbol as a logical location.
fn location(finding: &Finding) -> Option<Value> {
    let mut location = Map::new();
    if let Some(span) = &finding.location {
        location.insert(
            "physicalLocation".to_owned(),
            json!({
                "artifactLocation": {"uri": encoded(&span.file), "uriBaseId": SOURCE_ROOT},
                "region": region(span),
            }),
        );
    }
    if let Some(symbol) = &finding.symbol {
        location.insert(
            "logicalLocations".to_owned(),
            json!([{"fullyQualifiedName": symbol.display, "kind": "function"}]),
        );
    }
    (!location.is_empty()).then_some(Value::Object(location))
}

/// A SARIF region: 1-based lines and columns, the end column one past the last character, as
/// Cupel's own locations are.
fn region(span: &Location) -> Value {
    let mut region = json!({"startLine": span.line, "endLine": span.end_line});
    if let Some(col) = span.col {
        region["startColumn"] = json!(col);
    }
    if let Some(end_col) = span.end_col {
        region["endColumn"] = json!(end_col);
    }
    region
}

fn invocation(envelope: &Envelope) -> Value {
    let completed = matches!(envelope.status, Status::Pass | Status::Fail | Status::Plan);
    json!({
        "executionSuccessful": completed,
        "exitCode": envelope.exit_code.code(),
        "exitCodeDescription": envelope.exit_code.name(),
        "startTimeUtc": envelope.run.started,
        "properties": envelope.run,
    })
}

/// `cupel/<verb>/<run-id>`: the run's category, which tells one verb's logs from another's, and
/// its ID.
fn automation_id(envelope: &Envelope) -> String {
    let run = &envelope.run;
    match &run.verb {
        Some(verb) => format!("cupel/{}/{}", name_of(verb), run.id),
        None => format!("cupel/{}", run.id),
    }
}

/// The serialized name of a unit enum value, such as a verb or an action kind.
fn name_of(value: &impl serde::Serialize) -> String {
    match json!(value) {
        Value::String(name) => name,
        other => other.to_string(),
    }
}

/// `value` serialized as an object, without the fields `left_out`.
fn without(value: &impl serde::Serialize, left_out: &[&str]) -> Value {
    let mut fields = json!(value);
    if let Value::Object(fields) = &mut fields {
        for field in left_out {
            fields.remove(*field);
        }
    }
    fields
}

/// `root` as a `file://` URI of a directory, ending with a slash.
fn directory_uri(root: &Path) -> String {
    let path = encoded(&root.to_string_lossy());
    let slash = if path.ends_with('/') { "" } else { "/" };
    format!("file://{path}{slash}")
}

/// `path` with every byte but unreserved characters and `/` percent-encoded (RFC 3986).
fn encoded(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use cupel_core::{ExitCode, Finding, Severity, Status};
    use serde_json::{Value, json};

    use super::*;
    use crate::render::fixture;

    fn log(envelope: &Envelope, root: Option<&Path>) -> Value {
        let out = render(envelope, root);
        assert!(out.ends_with("}\n") && !out.ends_with("\n\n"), "{out}");
        serde_json::from_str(&out).expect("SARIF is JSON")
    }

    fn fixture_log() -> Value {
        log(&fixture::envelope(), Some(Path::new("/work/exonym")))
    }

    fn results(log: &Value) -> &Vec<Value> {
        log["runs"][0]["results"].as_array().expect("results")
    }

    fn rules(log: &Value) -> &Vec<Value> {
        log["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .expect("rules")
    }

    /// An envelope from a command line that named no verb: no git, scope, findings, or checks.
    fn bare_envelope() -> Envelope {
        let mut envelope = fixture::envelope();
        envelope.run.verb = None;
        envelope.run.git = None;
        envelope.run.scope = None;
        envelope.run.baseline = None;
        envelope.checks.clear();
        envelope.findings.clear();
        envelope.status = Status::Error;
        envelope.exit_code = ExitCode::Usage;
        envelope
    }

    #[test]
    fn the_log_is_valid_sarif_2_1_0() {
        for (envelope, root) in [
            (fixture::envelope(), Some(Path::new("/work/exonym"))),
            (fixture::envelope(), None),
            (bare_envelope(), None),
        ] {
            let log = log(&envelope, root);
            assert_eq!(
                cupel_fixtures::sarif_problems(&log),
                Vec::<String>::new(),
                "{log:#}"
            );
            assert_eq!(log["version"], json!("2.1.0"));
            assert_eq!(log["runs"].as_array().map(Vec::len), Some(1));
        }
    }

    #[test]
    fn the_fixture_envelope_renders_as_its_snapshot() {
        insta::assert_snapshot!(render(
            &fixture::envelope(),
            Some(Path::new("/work/exonym"))
        ));
    }

    #[test]
    fn the_driver_is_cupel_at_the_envelope_version() {
        let log = fixture_log();
        let driver = &log["runs"][0]["tool"]["driver"];
        assert_eq!(driver["name"], json!("cupel"));
        assert_eq!(driver["version"], json!("9.8.7"));
        assert_eq!(driver["semanticVersion"], json!("9.8.7"));
    }

    #[test]
    fn rules_are_the_kinds_of_the_checks_that_ran_then_every_other_finding_kind_once() {
        let catalog = crate::catalog::Catalog::builtin();
        let mutation = catalog
            .checks()
            .get(&"tests.mutation".into())
            .expect("tests.mutation is built in")
            .manifest()
            .findings
            .iter()
            .map(|spec| spec.kind.clone());
        // `perf.bench` ran too but is not built in, so it declares nothing; `crap.high` comes
        // from a finding of a check that is not in `checks`.
        let expected: Vec<Value> = mutation
            .chain(["crap.high".to_owned()])
            .map(Value::from)
            .collect();
        let log = fixture_log();

        let ids: Vec<&Value> = rules(&log).iter().map(|rule| &rule["id"]).collect();
        assert_eq!(ids, expected.iter().collect::<Vec<_>>());
    }

    #[test]
    fn a_rule_describes_its_kind_from_the_catalog() {
        let log = fixture_log();
        let rule = rules(&log)
            .iter()
            .find(|rule| rule["id"] == json!("crap.high"))
            .unwrap();
        let catalog = crate::catalog::Catalog::builtin();
        let described = catalog.finding("crap.high").unwrap();
        let check = catalog.checks().get(&"static.crap".into()).unwrap();

        assert_eq!(rule["shortDescription"]["text"], json!(described.summary));
        assert_eq!(
            rule["fullDescription"]["text"],
            json!(check.manifest().summary)
        );
        let help = rule["help"]["text"].as_str().unwrap();
        assert!(help.starts_with(&described.summary), "{help}");
        assert!(
            help.contains("`cupel describe finding crap.high`"),
            "{help}"
        );
        assert!(help.contains("`cupel describe static.crap`"), "{help}");
        assert_eq!(
            rule["properties"],
            json!({
                "check": "static.crap",
                "action": "write_test",
                "tier": 0,
                "cost": "quick",
                "stability": "experimental",
            })
        );
    }

    #[test]
    fn a_kind_outside_the_catalog_still_has_a_rule() {
        let mut envelope = bare_envelope();
        envelope.findings = vec![fixture::finding(
            0,
            "plugin.lint",
            "lint.odd",
            Severity::Warn,
        )];
        let log = log(&envelope, None);
        let rule = &rules(&log)[0];

        assert_eq!(rule["id"], json!("lint.odd"));
        assert_eq!(
            rule["shortDescription"]["text"],
            json!("A `lint.odd` finding of plugin.lint.")
        );
        assert!(rule.get("fullDescription").is_none(), "{rule:#}");
        assert_eq!(rule["properties"], json!({"check": "plugin.lint"}));
        assert_eq!(cupel_fixtures::sarif_problems(&log), Vec::<String>::new());
    }

    #[test]
    fn each_finding_is_a_result_pointing_at_its_rule() {
        let log = fixture_log();
        let kinds: Vec<Value> = results(&log)
            .iter()
            .map(|result| {
                let index = usize::try_from(result["ruleIndex"].as_u64().unwrap()).unwrap();
                assert_eq!(rules(&log)[index]["id"], result["ruleId"]);
                result["ruleId"].clone()
            })
            .collect();
        assert_eq!(
            kinds,
            [
                json!("mutant.survived"),
                json!("crap.high"),
                json!("mutant.timeout")
            ]
        );
    }

    #[test]
    fn the_level_follows_the_severity() {
        let mut envelope = bare_envelope();
        envelope.findings = [Severity::Fail, Severity::Warn, Severity::Info]
            .into_iter()
            .enumerate()
            .map(|(n, severity)| fixture::finding(u32::try_from(n).unwrap(), "x", "x.y", severity))
            .collect();
        let levels: Vec<Value> = results(&log(&envelope, None))
            .iter()
            .map(|result| result["level"].clone())
            .collect();
        assert_eq!(levels, [json!("error"), json!("warning"), json!("note")]);
    }

    #[test]
    fn a_result_carries_its_message_location_symbol_and_fingerprints() {
        let log = fixture_log();
        let result = &results(&log)[0];
        let finding = &fixture::envelope().findings[0];

        assert_eq!(
            result["message"]["text"],
            json!(format!(
                "{}\n\n{}",
                finding.title,
                finding.detail.as_deref().unwrap()
            ))
        );
        assert_eq!(
            result["locations"],
            json!([{
                "physicalLocation": {
                    "artifactLocation": {"uri": "src/rewrite/planner.rs", "uriBaseId": "SRCROOT"},
                    "region": {"startLine": 142, "startColumn": 21, "endLine": 144, "endColumn": 23},
                },
                "logicalLocations": [{
                    "fullyQualifiedName": "exonym::rewrite::Planner::push_limit_0",
                    "kind": "function",
                }],
            }])
        );
        let symbol = finding.symbol.as_ref().unwrap();
        assert_eq!(
            result["partialFingerprints"],
            json!({
                "cupel/findingId": finding.id.to_string(),
                "cupel/symbolHash": symbol.hash.to_string(),
            })
        );
        assert_eq!(result["fixes"], json!([]));
    }

    #[test]
    fn a_result_without_location_or_symbol_has_no_locations() {
        let mut finding: Finding = fixture::finding(0, "cli", "cli.usage", Severity::Fail);
        finding.location = None;
        finding.symbol = None;
        finding.detail = None;
        let mut envelope = bare_envelope();
        envelope.findings = vec![finding.clone()];
        let log = log(&envelope, None);
        let result = &results(&log)[0];

        assert!(result.get("locations").is_none(), "{result:#}");
        assert_eq!(result["message"]["text"], json!(finding.title));
        assert_eq!(
            result["partialFingerprints"],
            json!({"cupel/findingId": finding.id.to_string()})
        );
    }

    #[test]
    fn a_location_without_columns_is_a_line_region() {
        let mut finding = fixture::finding(0, "x", "x.y", Severity::Warn);
        let location = finding.location.as_mut().unwrap();
        location.col = None;
        location.end_col = None;
        location.file = "dir with space/naïve%.rs".to_owned();
        let mut envelope = bare_envelope();
        envelope.findings = vec![finding];
        let log = log(&envelope, None);
        let physical = &results(&log)[0]["locations"][0]["physicalLocation"];

        assert_eq!(
            physical["region"],
            json!({"startLine": 142, "endLine": 144})
        );
        assert_eq!(
            physical["artifactLocation"]["uri"],
            json!("dir%20with%20space/na%C3%AFve%25.rs")
        );
    }

    #[test]
    fn a_suppressed_finding_has_an_accepted_external_suppression() {
        let log = fixture_log();
        let suppressions: Vec<Value> = results(&log)
            .iter()
            .map(|result| result["suppressions"].clone())
            .collect();
        assert_eq!(
            suppressions,
            [
                json!([]),
                json!([]),
                json!([{"kind": "external", "status": "accepted"}])
            ]
        );
    }

    #[test]
    fn the_baseline_state_is_set_only_against_a_baseline() {
        let states = |envelope: &Envelope| -> Vec<Value> {
            results(&log(envelope, None))
                .iter()
                .map(|result| result.get("baselineState").cloned().unwrap_or(Value::Null))
                .collect()
        };
        assert_eq!(
            states(&fixture::envelope()),
            [json!("new"), json!("unchanged"), json!("new")]
        );
        let mut without = fixture::envelope();
        without.run.baseline = None;
        assert_eq!(states(&without), [Value::Null, Value::Null, Value::Null]);
    }

    #[test]
    fn a_result_keeps_what_sarif_has_no_place_for_in_its_properties() {
        let log = fixture_log();
        let finding = serde_json::to_value(&fixture::envelope().findings[0]).unwrap();
        let mut expected = finding.as_object().unwrap().clone();
        for mapped in [
            "id",
            "severity",
            "title",
            "detail",
            "location",
            "suppressed",
        ] {
            expected.remove(mapped);
        }
        assert_eq!(results(&log)[0]["properties"], Value::Object(expected));
    }

    #[test]
    fn the_invocation_reports_the_exit_and_keeps_the_run_whole() {
        let log = fixture_log();
        let invocation = &log["runs"][0]["invocations"][0];
        let envelope = fixture::envelope();

        assert_eq!(invocation["executionSuccessful"], json!(true));
        assert_eq!(invocation["exitCode"], json!(1));
        assert_eq!(invocation["exitCodeDescription"], json!("fail"));
        assert_eq!(invocation["startTimeUtc"], json!("2026-10-10T16:31:04Z"));
        assert_eq!(
            invocation["properties"],
            serde_json::to_value(&envelope.run).unwrap()
        );
        let failed = log_of_status(Status::Error, ExitCode::Environment);
        assert_eq!(
            failed["runs"][0]["invocations"][0]["executionSuccessful"],
            json!(false)
        );
        let partial = log_of_status(Status::Partial, ExitCode::Partial);
        assert_eq!(
            partial["runs"][0]["invocations"][0]["executionSuccessful"],
            json!(false)
        );
    }

    fn log_of_status(status: Status, exit_code: ExitCode) -> Value {
        let mut envelope = fixture::envelope();
        envelope.status = status;
        envelope.exit_code = exit_code;
        log(&envelope, None)
    }

    #[test]
    fn the_run_is_identified_by_verb_and_run_id() {
        assert_eq!(
            fixture_log()["runs"][0]["automationDetails"],
            json!({"id": "cupel/check/run_01JA2B3C4D5E6F7G8H9J0KMNPQ"})
        );
        let bare = log(&bare_envelope(), None);
        assert_eq!(
            bare["runs"][0]["automationDetails"],
            json!({"id": "cupel/run_01JA2B3C4D5E6F7G8H9J0KMNPQ"})
        );
    }

    #[test]
    fn the_root_is_the_source_base_and_the_repository_of_the_revision() {
        let log = log(&fixture::envelope(), Some(Path::new("/work/my repo")));
        let run = &log["runs"][0];

        assert_eq!(
            run["originalUriBaseIds"],
            json!({"SRCROOT": {"uri": "file:///work/my%20repo/"}})
        );
        assert_eq!(
            run["versionControlProvenance"],
            json!([{
                "repositoryUri": "file:///work/my%20repo/",
                "revisionId": "9f1c2d3",
                "branch": "feat/limit-pushdown",
                "mappedTo": {"uriBaseId": "SRCROOT"},
            }])
        );
    }

    #[test]
    fn without_a_root_or_git_there_is_no_base_or_revision() {
        let unrooted = log(&fixture::envelope(), None);
        assert!(unrooted["runs"][0].get("originalUriBaseIds").is_none());
        assert!(
            unrooted["runs"][0]
                .get("versionControlProvenance")
                .is_none()
        );
        let mut envelope = fixture::envelope();
        envelope.run.git = None;
        let ungitted = log(&envelope, Some(Path::new("/work/exonym")));
        assert!(ungitted["runs"][0].get("originalUriBaseIds").is_some());
        assert!(
            ungitted["runs"][0]
                .get("versionControlProvenance")
                .is_none()
        );
    }

    #[test]
    fn the_run_keeps_the_rest_of_the_envelope_in_its_properties() {
        let envelope = fixture::envelope();
        let mut expected = serde_json::to_value(&envelope)
            .unwrap()
            .as_object()
            .unwrap()
            .clone();
        expected.remove("run");
        expected.remove("findings");
        assert_eq!(
            fixture_log()["runs"][0]["properties"],
            Value::Object(expected)
        );
    }
}
