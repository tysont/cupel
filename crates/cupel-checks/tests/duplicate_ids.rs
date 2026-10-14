// ABOUTME: A second check submitted under a registered ID makes `register` panic (ADR-074). It is
// ABOUTME: its own test binary because the submission joins every registry this binary builds.
use cupel_core::{
    CheckCtx, CheckId, CheckRegistry, CheckResult, CostClass, EventSink, Facts, Manifest, Plan,
    Result, Scope, Stability, Tier,
};

/// A check that only claims an ID already taken by a native check.
struct Twin(Manifest);

impl cupel_core::Check for Twin {
    fn manifest(&self) -> &Manifest {
        &self.0
    }

    fn plan(&self, _: &CheckCtx, _: &Scope) -> Result<Plan> {
        unreachable!("registration never plans")
    }

    fn run(&self, _: &CheckCtx, _: &Facts, _: &Plan, _: &mut dyn EventSink) -> Result<CheckResult> {
        unreachable!("registration never runs")
    }
}

inventory::submit!(cupel_checks::NativeCheck(|| Box::new(Twin(Manifest {
    id: CheckId::from("static.complexity"),
    version: 1,
    tier: Tier::try_from(0).unwrap(),
    cost: CostClass::Quick,
    needs: vec![],
    wants: vec![],
    findings: vec![],
    metrics: vec![],
    stability: Stability::Experimental,
    summary: String::new(),
}))));

#[test]
#[should_panic(expected = "native check IDs are unique")]
fn a_second_submission_with_a_registered_id_panics() {
    cupel_checks::register(&mut CheckRegistry::default());
}
