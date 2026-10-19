// ABOUTME: The `survey` verb (design §6.5, ADR-062, ADR-132): `check`'s pipeline over the same scope and depth,
// ABOUTME: no gate, plus `rankings`: the worst targets of every per-target metric, and what to protect first.
use cupel_core::{Envelope, EventSink, RANKED, Status, rankings};

use crate::check;
use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::run::Invocation;
use crate::suppress;

/// `cupel survey`: the envelope of running the selected checks with their rankings, or of
/// planning them.
pub fn verb(
    invocation: &Invocation<'_>,
    args: &ScopeArgs,
    events: &mut dyn EventSink,
) -> Result<Envelope, CupelError> {
    let config = invocation.config()?;
    if args.gate {
        return Err(CupelError::bad_argument(
            "--gate",
            None,
            "`survey` ranks without gating, so it takes no `--gate`".to_owned(),
            "Run `cupel check --gate` to evaluate the gate.".to_owned(),
        ));
    }
    let mut checked = check::run(invocation, config, args, events)?;
    let (manifests, measured) = checked.take_measurements();
    let mut envelope = checked.into_envelope(invocation.run, invocation.writer, events);
    if envelope.status != Status::Plan {
        // A suppressed finding recommends nothing (ADR-132); grading applies the same
        // suppressions again, which changes nothing.
        suppress::apply(&mut envelope, invocation.writer)?;
        envelope.rankings = Some(rankings(&manifests, &measured, &envelope.findings, RANKED));
    }
    Ok(envelope)
}
