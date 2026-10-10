// ABOUTME: The resolved scope a check plans over and a producer produces for.
// ABOUTME: Scope resolution itself (--changed, --paths, --symbols) arrives in P2.1.
/// The files a run considers, relative to the repository root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    pub files: Vec<String>,
}
