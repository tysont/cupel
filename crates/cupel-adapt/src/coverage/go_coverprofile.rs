// ABOUTME: The `go test -coverprofile` coverage producer for Go: the configured command, whose profile names files
// ABOUTME: by import path, mapped to paths through the root's `go.mod` and `go.work`; it measures no branches.
use cupel_core::coverage::CoverageMap;

use super::tool::{CoverageSpec, CoverageTool, every_file};
use crate::go_module::Modules;

const ID: &str = "tool.go-coverprofile";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["go"],
    tool: "go",
    node_bin: false,
    version: &["go", "version"],
    default_report: None,
    resolve: module_paths,
    listing: every_file,
    providers: &[],
    uses: None,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});

/// `map` with each file's import path (`example.com/grade/grade.go`) made relative to the
/// root `dir` through its modules (its `go.mod`, and each module its `go.work` uses); files of
/// other modules are dropped.
pub(crate) fn module_paths(map: CoverageMap, dir: &std::path::Path) -> CoverageMap {
    let modules = Modules::of(dir);
    CoverageMap::new(
        map.files
            .into_iter()
            .filter_map(|mut file| {
                file.file = modules.file(&file.file)?;
                Some(file)
            })
            .collect(),
    )
}
