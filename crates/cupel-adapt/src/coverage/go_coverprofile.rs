// ABOUTME: The `go test -coverprofile` coverage producer for Go: the configured command, whose profile names files
// ABOUTME: by import path, mapped to paths through the root's `go.mod`; coverprofile measures no branches.
use cupel_core::coverage::CoverageMap;

use super::tool::{CoverageSpec, CoverageTool};

const ID: &str = "tool.go-coverprofile";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["go"],
    tool: "go",
    node_bin: false,
    version: &["go", "version"],
    default_report: None,
    resolve: module_paths,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});

/// `map` with each file's import path (`example.com/grade/grade.go`) made relative to the
/// module root `dir` through its `go.mod`; files of other modules are dropped.
pub(crate) fn module_paths(map: CoverageMap, dir: &std::path::Path) -> CoverageMap {
    let module = std::fs::read_to_string(dir.join("go.mod"))
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|line| line.trim().strip_prefix("module "))
                .map(|module| format!("{}/", module.trim().trim_matches('"')))
        })
        .unwrap_or_default();
    CoverageMap::new(
        map.files
            .into_iter()
            .filter_map(|mut file| {
                file.file = file.file.strip_prefix(&module)?.to_owned();
                Some(file)
            })
            .collect(),
    )
}
