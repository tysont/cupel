// Three accessors that map a kind's bare constants to names in one `switch`, `case Reference:`: a
// clone cluster reported at `info` with the shape `enum-mapping` (ADR-158 §3).

/** Where an oracle's expected values come from. */
export const Reference = 0;
export const Command = 1;
export const Recorded = 2;
export const Generated = 3;

/** The source's name in configuration. */
export function sourceName(source: number): string {
  switch (source) {
    case Reference:
      return "reference";
    case Command:
      return "command";
    case Recorded:
      return "recorded";
    case Generated:
      return "generated";
  }
  return "";
}

/** A phase of a suite run. */
export const Build = 0;
export const Test = 1;
export const Measure = 2;
export const Report = 3;

/** The phase's name in the envelope. */
export function phaseName(phase: number): string {
  switch (phase) {
    case Build:
      return "build";
    case Test:
      return "test";
    case Measure:
      return "measure";
    case Report:
      return "report";
  }
  return "";
}

/** How a run reused an earlier one. */
export const Fresh = 0;
export const Cached = 1;
export const Partial = 2;
export const Stale = 3;

/** The reuse's name in the run record. */
export function reuseName(reuse: number): string {
  switch (reuse) {
    case Fresh:
      return "fresh";
    case Cached:
      return "cached";
    case Partial:
      return "partial";
    case Stale:
      return "stale";
  }
  return "";
}
