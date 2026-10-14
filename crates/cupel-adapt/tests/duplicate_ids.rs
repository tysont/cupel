// ABOUTME: Two tool producers submitted under one ID make `cupel_adapt::register` panic (ADR-074). It
// ABOUTME: is its own test binary because the submissions join every registry this binary builds.
use cupel_adapt::ToolProducer;
use cupel_core::ProducerRegistry;

inventory::submit!(ToolProducer {
    id: "tool.twin",
    register: |_| {},
});

inventory::submit!(ToolProducer {
    id: "tool.twin",
    register: |_| {},
});

#[test]
#[should_panic(expected = "tool producer IDs are unique: `tool.twin` is submitted twice")]
fn two_submissions_with_one_id_panic() {
    cupel_adapt::register(&mut ProducerRegistry::default());
}
