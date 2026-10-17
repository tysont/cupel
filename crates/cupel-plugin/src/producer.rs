// ABOUTME: A producer plugin as a `Producer` of the kind it provides (ADR-089): upstream facts written as files, the
// ABOUTME: plugin run, its one fact checked against the kind's type and version. One table maps each kind to its type.
use std::marker::PhantomData;

use cupel_core::coverage::CoverageMap;
use cupel_core::plugin::ProducerManifest;
use cupel_core::syntax::{
    ApiSurface, Decisions, ModuleGraph, MutationSites, NormTree, ProjectInfo, Roots, Symbols,
    TestConventions, TestFrames,
};
use cupel_core::{
    Availability, CostClass, Ctx, DivergenceResults, EventSink, Fact, FactKind, Langs,
    MutationOutcomes, PerTestMap, ProduceCtx, Producer, ProducerId, ProducerRegistry, Result,
    Scope, TestReruns, TestResults,
};

use crate::command::PluginCommand;
use crate::output;
use crate::request::{self, Inputs};

/// Registers one producer plugin as a producer of `F`.
type Register = fn(&mut ProducerRegistry, ProducerManifest, PluginCommand);

/// Every kind a plugin may provide (those with a payload type, ADR-088 §8), with its registration.
const KINDS: [(FactKind, Register); 16] = [
    (FactKind::Symbols, register_as::<Symbols>),
    (FactKind::Decisions, register_as::<Decisions>),
    (FactKind::NormTree, register_as::<NormTree>),
    (FactKind::MutationSites, register_as::<MutationSites>),
    (FactKind::TestFrames, register_as::<TestFrames>),
    (FactKind::ModuleGraph, register_as::<ModuleGraph>),
    (FactKind::ApiSurface, register_as::<ApiSurface>),
    (FactKind::Roots, register_as::<Roots>),
    (FactKind::TestConventions, register_as::<TestConventions>),
    (FactKind::ProjectInfo, register_as::<ProjectInfo>),
    (FactKind::TestResults, register_as::<TestResults>),
    (FactKind::TestReruns, register_as::<TestReruns>),
    (FactKind::CoverageMap, register_as::<CoverageMap>),
    (FactKind::PerTestMap, register_as::<PerTestMap>),
    (FactKind::MutationOutcomes, register_as::<MutationOutcomes>),
    (
        FactKind::DivergenceResults,
        register_as::<DivergenceResults>,
    ),
];

/// The kinds a producer plugin may provide, in catalog order.
pub fn providable_kinds() -> Vec<FactKind> {
    KINDS.iter().map(|(kind, _)| *kind).collect()
}

/// Registers the producer plugin `manifest` describes; discovery admits only providable kinds.
pub(crate) fn register(
    registry: &mut ProducerRegistry,
    manifest: ProducerManifest,
    command: PluginCommand,
) {
    if let Some((_, register)) = KINDS
        .iter()
        .find(|(kind, _)| *kind == manifest.provides.kind)
    {
        register(registry, manifest, command);
    }
}

fn register_as<F: Fact>(
    registry: &mut ProducerRegistry,
    manifest: ProducerManifest,
    command: PluginCommand,
) {
    registry.register::<F, _>(PluginProducer::<F> {
        manifest,
        command,
        fact: PhantomData,
    });
}

/// A producer implemented by a plugin, registered after the native producers (ADR-088 §7).
struct PluginProducer<F> {
    manifest: ProducerManifest,
    command: PluginCommand,
    fact: PhantomData<fn() -> F>,
}

impl<F: Fact> Producer<F> for PluginProducer<F> {
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

    /// Ready: the plugin answered `describe` (ADR-088 §7).
    fn availability(&self, _ctx: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, _sink: &mut dyn EventSink) -> Result<F> {
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
            output::fact_output::<F>(&self.manifest, stdout)
        })
        .map(|(fact, _)| fact)
    }
}
