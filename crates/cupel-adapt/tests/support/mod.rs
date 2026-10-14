// ABOUTME: Helpers the tool-producer tests share: the registry `cupel_adapt::register` fills, languages, the
// ABOUTME: context a producer gets for a fixture copy, and finding a registered producer by ID.
#![allow(
    dead_code,
    reason = "each test binary uses a different subset of these helpers"
)]

use std::path::Path;

use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{
    Cancellation, Ctx, FactKind, Lang, LangFamily, LangId, ProduceCtx, Produced, ProducerRegistry,
    RegisteredProducer, Writer,
};

pub fn registry() -> ProducerRegistry {
    let mut registry = ProducerRegistry::default();
    cupel_adapt::register(&mut registry);
    registry
}

pub fn lang(id: &str) -> Lang {
    let family = if matches!(id, "javascript" | "typescript") {
        "ecma"
    } else {
        id
    };
    Lang {
        id: LangId::from(id),
        family: LangFamily::from(family),
    }
}

/// The registered `TestResults` producer `id` for `lang`.
pub fn runner<'a>(
    registry: &'a ProducerRegistry,
    id: &str,
    lang: &str,
) -> &'a dyn RegisteredProducer {
    producer(registry, FactKind::TestResults, id, lang)
}

/// The registered producer `id` of `kind` for `lang`.
pub fn producer<'a>(
    registry: &'a ProducerRegistry,
    kind: FactKind,
    id: &str,
    lang: &str,
) -> &'a dyn RegisteredProducer {
    registry
        .producers_for(kind, &self::lang(lang))
        .find(|producer| producer.id().as_str() == id)
        .unwrap()
}

/// The context a runner gets for the repository at `root`: one project root, `.`.
pub fn produce_ctx(root: &Path, lang: &str, cancel: Cancellation) -> ProduceCtx {
    let mut ctx = ProduceCtx::new(
        Ctx::new(root.to_path_buf()),
        Writer::new(root, false).unwrap().confined(),
        cancel,
    );
    ctx.upstream.insert(
        Produced::new(ProjectInfo {
            roots: vec![ProjectRoot {
                path: ".".to_owned(),
                lang: LangId::from(lang),
                build_files: Vec::new(),
                members: Vec::new(),
                packages: if lang == "go" {
                    vec!["./...".to_owned()]
                } else {
                    Vec::new()
                },
                skip_dirs: Vec::new(),
            }],
        })
        .share(),
    );
    ctx
}
