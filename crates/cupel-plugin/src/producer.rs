// ABOUTME: A producer plugin as a producer of the kind it provides (ADR-089): upstream facts written as files, the
// ABOUTME: plugin run, its one fact checked against the kind's version and decoded by the kind's catalog entry (ADR-123).
use cupel_core::plugin::ProducerManifest;
use cupel_core::{
    Availability, CostClass, Ctx, Estimate, EventSink, FactEntry, FactKind, Langs, ProduceCtx,
    Produced, ProducerId, ProducerRegistry, RegisteredProducer, Result, Scope, ToolProbe, Upstream,
};

use crate::command::PluginCommand;
use crate::output;
use crate::request::{self, Inputs};

/// The kinds a producer plugin may provide (every catalog kind, ADR-088 §8), in catalog order.
pub fn providable_kinds() -> Vec<FactKind> {
    FactEntry::catalog()
        .iter()
        .map(|entry| entry.kind)
        .collect()
}

/// Registers the producer plugin `manifest` describes; discovery admits only providable kinds.
pub(crate) fn register(
    registry: &mut ProducerRegistry,
    manifest: ProducerManifest,
    command: PluginCommand,
) {
    let entry = manifest.provides.kind.entry();
    registry.register_dyn(Box::new(PluginProducer {
        manifest,
        command,
        entry,
    }));
}

/// A producer implemented by a plugin, registered after the native producers (ADR-088 §7).
struct PluginProducer {
    manifest: ProducerManifest,
    command: PluginCommand,
    /// The catalog entry of the kind it provides, which decodes its fact.
    entry: &'static FactEntry,
}

impl RegisteredProducer for PluginProducer {
    fn provides(&self) -> FactKind {
        self.entry.kind
    }

    fn id(&self) -> ProducerId {
        self.manifest.id.clone()
    }

    fn languages(&self) -> Langs {
        Langs::Only(self.manifest.languages.clone())
    }

    fn needs(&self) -> &[FactKind] {
        &self.manifest.needs
    }

    fn wants(&self) -> &[FactKind] {
        &self.manifest.wants
    }

    fn cost(&self) -> CostClass {
        self.manifest.cost
    }

    fn estimate(&self, _ctx: &Ctx, _upstream: &Upstream) -> Estimate {
        Estimate::of(self.manifest.cost)
    }

    /// Ready: the plugin answered `describe` (ADR-088 §7).
    fn availability(&self, _ctx: &Ctx) -> Availability {
        Availability::Ready
    }

    fn detected(&self, _ctx: &Ctx) -> bool {
        true
    }

    fn tool(&self, _ctx: &Ctx) -> Option<ToolProbe> {
        None
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _sink: &mut dyn EventSink,
    ) -> Result<Produced> {
        let scratch = ctx
            .writer
            .cupel_dir()
            .join("work")
            .join("plugins")
            .join(self.manifest.id.as_str());
        let inputs = self.manifest.needs.iter().chain(&self.manifest.wants);
        let files = request::write_facts(
            &ctx.writer,
            &scratch,
            inputs.filter_map(|kind| ctx.upstream.json(*kind).map(|json| (*kind, json))),
        )?;
        let request = request::request(
            &Inputs {
                root: &ctx.ctx.root,
                scope,
                config: &ctx.config,
                cancel: &ctx.cancel,
                scratch: &scratch,
                run: Some((ctx.focus.depth, ctx.seed)),
            },
            files,
        );
        let captured = self.command.call("run", Some(&request), &ctx.cancel)?;
        output::accepted(&self.command, &captured, "fact", |stdout| {
            output::fact_output(&self.manifest, self.entry, stdout)
        })
        .map(|(fact, _)| fact)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use cupel_core::plugin::Provides;
    use cupel_core::{Lang, LangFamily, LangId, Stability};

    use super::*;

    fn manifest(kind: FactKind) -> ProducerManifest {
        ProducerManifest {
            id: ProducerId::from("plugin.sample"),
            version: 1,
            provides: Provides {
                kind,
                version: kind.entry().version,
            },
            languages: vec![LangId::from("typescript")],
            needs: Vec::new(),
            wants: Vec::new(),
            cost: CostClass::Quick,
            stability: Stability::Experimental,
            summary: String::new(),
        }
    }

    #[test]
    fn every_providable_kind_registers_a_producer_of_that_kind() {
        let typescript = Lang {
            id: LangId::from("typescript"),
            family: LangFamily::from("ecmascript"),
        };
        let command = PluginCommand::new("sample", "sample", &[], Path::new("."));
        let unregistered: Vec<FactKind> = providable_kinds()
            .into_iter()
            .filter(|kind| {
                let mut registry = ProducerRegistry::default();
                register(&mut registry, manifest(*kind), command.clone());
                let provided: Vec<FactKind> = registry
                    .producers_for(*kind, &typescript)
                    .map(RegisteredProducer::provides)
                    .collect();
                provided != [*kind]
            })
            .collect();
        assert_eq!(unregistered, Vec::<FactKind>::new());
    }

    /// Registration goes through the catalog, so this module holds no per-kind table (ADR-123).
    #[test]
    fn this_module_names_no_fact_kind() {
        assert!(!include_str!("producer.rs").contains(concat!("FactKind", "::")));
    }
}
