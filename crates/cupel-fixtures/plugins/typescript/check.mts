// ABOUTME: The TypeScript reference check plugin (ADR-092): reports each file in scope that no test in the per-test
// ABOUTME: map executes, as `ref.untested-file`; speaks the plugin protocol (design §8.6) with Node's standard library.
import { readFileSync } from "node:fs";

const CHECK = "ref.untested-typescript";
const KIND = "ref.untested-file";

type Covered = { file: string; lines: number[] };
type PerTestMap = { tests: { covered: Covered[] }[] };
type Request = { scope: { files: string[] }; facts: Record<string, string> };

function manifest(): object {
  return {
    kind: "check",
    id: CHECK,
    version: 1,
    tier: 2,
    cost: "quick",
    needs: ["per-test-map"],
    wants: [],
    findings: [
      {
        kind: KIND,
        action: "write_test",
        summary: "A file in scope that no test executes.",
        evidence: {
          type: "object",
          properties: { file: { type: "string" } },
          required: ["file"],
          additionalProperties: false,
        },
      },
    ],
    metrics: [{ name: "untested", summary: "Files in scope no test executes.", per: "scope" }],
    stability: "experimental",
    summary: "Files in scope that no test executes, from the per-test map.",
  };
}

/** `fnd_` and six hex digits of 32-bit FNV-1a over the check, kind, and file joined by NUL. */
function findingId(file: string): string {
  let digest = 0x811c9dc5;
  for (const byte of new TextEncoder().encode(`${CHECK}\0${KIND}\0${file}`)) {
    digest = Math.imul(digest ^ byte, 0x01000193) >>> 0;
  }
  return `fnd_${digest.toString(16).padStart(8, "0").slice(0, 6)}`;
}

function finding(file: string): object {
  const id = findingId(file);
  return {
    ev: "finding",
    id,
    check: CHECK,
    kind: KIND,
    severity: "warn",
    confidence: 1.0,
    tier: 2,
    symbol: null,
    location: { file, line: 1, col: null, end_line: 1, end_col: null },
    title: `No test executes ${file}`,
    detail: null,
    evidence: { file },
    action: {
      kind: "write_test",
      priority: 1,
      summary: `Write a test that executes ${file}.`,
      hints: {},
    },
    repro: `cupel check --only ${CHECK}`,
    suppress: `cupel suppress ${id} --reason '…'`,
    new_since_baseline: false,
    suppressed: false,
  };
}

function run(request: Request): void {
  const perTest: PerTestMap = JSON.parse(readFileSync(request.facts["per-test-map"], "utf8"));
  const covered = new Set<string>();
  for (const test of perTest.tests) {
    for (const lines of test.covered) {
      if (lines.lines.length > 0) covered.add(lines.file);
    }
  }
  const files = request.scope.files;
  const untested = files.filter((file) => !covered.has(file));
  for (const file of untested) emit(finding(file));
  const tested = files.length - untested.length;
  emit({
    ev: "result",
    status: untested.length > 0 ? "fail" : "pass",
    score: {
      value: files.length > 0 ? tested / files.length : 1.0,
      unit: "tested_files",
      n: files.length,
    },
    metrics: { untested: untested.length },
    notes: [],
  });
}

function emit(message: object): void {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

function request(): Request {
  return JSON.parse(readFileSync(0, "utf8"));
}

const command = process.argv[2] ?? "";
if (command === "describe") {
  emit(manifest());
} else if (command === "plan") {
  const files = request().scope.files;
  emit({ targets: files.length, est_ms: files.length });
} else if (command === "run") {
  run(request());
} else {
  process.stderr.write(`usage: check.mts describe|plan|run, not \`${command}\`\n`);
  process.exit(64);
}
