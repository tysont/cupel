// ABOUTME: The repository's project roots from every registered language's `ProjectInfo` producer,
// ABOUTME: joined with the cupel-adapt toolchain table to choose each root's tools and commands (ADR-037).
use std::path::Path;

use cupel_adapt::toolchain::{self, Commands, Root, Selection};
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Cancellation, Ctx, Event, FactKind, ProduceCtx, Produced, ProducerRegistry, Scope, Writer,
};
use cupel_lang::LanguageRegistry;

/// Every registered language's project roots in the repository at `writer`'s root, ordered by
/// path, then language. Producers get `writer` confined to `.cupel/`. A producer that fails
/// contributes nothing: detection is best effort.
pub fn detect(writer: &Writer) -> ProjectInfo {
    let languages = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let ctx = ProduceCtx {
        ctx: Ctx {
            root: writer.root().to_path_buf(),
        },
        config: serde_json::Map::new(),
        writer: writer.confined(),
        cancel: Cancellation::new(),
    };
    let mut roots: Vec<ProjectRoot> = languages
        .langs()
        .iter()
        .filter_map(|lang| producers.producers_for(FactKind::ProjectInfo, lang).next())
        .filter_map(|producer| {
            let mut events: Vec<Event> = Vec::new();
            producer
                .produce(&ctx, &Scope::default(), &mut events)
                .ok()
                .and_then(Produced::downcast::<ProjectInfo>)
        })
        .flat_map(|info| info.roots)
        .collect();
    roots.sort_by(|a, b| (&a.path, &a.lang).cmp(&(&b.path, &b.lang)));
    ProjectInfo { roots }
}

/// The toolchain-table tools for `root`, a root of the repository at `repo`; `None` when the
/// table has no row for its language.
pub fn selection(repo: &Path, root: &ProjectRoot) -> Option<Selection<'static>> {
    let row = toolchain::for_lang(&root.lang)?;
    let dir = repo.join(&root.path);
    let members = root.members.iter().map(|member| dir.join(member)).collect();
    Some(row.select(&Root::new(dir, members, root.packages.clone())))
}

/// The default commands for `root`; none when the table has no row for its language.
pub fn commands(repo: &Path, root: &ProjectRoot) -> Commands {
    selection(repo, root).map_or_else(Commands::default, |selection| selection.commands())
}
