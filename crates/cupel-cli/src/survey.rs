// ABOUTME: The `survey` verb (design §6.5, ADR-062): `check`'s pipeline over the same scope and depth, with
// ABOUTME: no gate, plus `rankings`: the worst targets of every per-target metric the checks declare.
use cupel_core::{Envelope, EventSink, Status};

use crate::check;
use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::run::Invocation;

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
    let checked = check::run(invocation, config, args, events)?;
    let rankings = checked.rankings();
    let mut envelope = checked.into_envelope(invocation.run, invocation.writer, events);
    if envelope.status != Status::Plan {
        envelope.rankings = Some(rankings);
    }
    Ok(envelope)
}
