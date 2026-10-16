// ABOUTME: The TypeScript reference producer plugin (ADR-090): a `per-test-map` fact for JavaScript and TypeScript read
// ABOUTME: from a committed LCOV file with one `TN:` section per test; speaks the plugin protocol with Node's standard library.
import { readFileSync } from "node:fs";
import { join } from "node:path";

const PRODUCER = "plugin.ref-typescript";
const DEFAULT_LCOV = "coverage/per-test.lcov";

type Request = { root: string; config: Record<string, unknown> };
type TestCoverage = { id: string; file: null; covered: { file: string; lines: number[] }[] };

function manifest(): object {
  return {
    kind: "producer",
    id: PRODUCER,
    version: 1,
    provides: { kind: "per-test-map", version: 2 },
    languages: ["javascript", "typescript"],
    needs: [],
    wants: [],
    cost: "quick",
    stability: "experimental",
    summary: "The lines each test executes, from an LCOV file with a TN: section per test.",
  };
}

/** Each `TN:` test's covered lines, by file: `DA:<line>,<hits>` records with hits above zero. */
function perTest(text: string): TestCoverage[] {
  const tests = new Map<string, Map<string, Set<number>>>();
  let test: string | null = null;
  let file: string | null = null;
  for (const record of text.split("\n")) {
    const colon = record.indexOf(":");
    const key = colon < 0 ? record : record.slice(0, colon);
    const value = colon < 0 ? "" : record.slice(colon + 1);
    if (key === "TN") {
      test = value;
      if (!tests.has(test)) tests.set(test, new Map());
    } else if (key === "SF") {
      file = value;
    } else if (key === "DA" && test !== null && file !== null) {
      const [line, hits] = value.split(",").map(Number);
      const files = tests.get(test)!;
      if (hits > 0) {
        if (!files.has(file)) files.set(file, new Set());
        files.get(file)!.add(line);
      }
    }
  }
  const byName = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
  return [...tests.keys()].sort(byName).map((id) => {
    const files = tests.get(id)!;
    return {
      id,
      file: null,
      covered: [...files.keys()].sort(byName).map((name) => ({
        file: name,
        lines: [...files.get(name)!].sort((a, b) => a - b),
      })),
    };
  });
}

function run(request: Request): void {
  const configured = request.config["per_test_lcov"];
  const path = join(request.root, typeof configured === "string" ? configured : DEFAULT_LCOV);
  let tests: TestCoverage[];
  try {
    tests = perTest(readFileSync(path, "utf8"));
  } catch (error) {
    process.stderr.write(`cannot read the per-test LCOV file: ${error}\n`);
    process.exit(1);
  }
  const payload = { runner: "tool.vitest", tests, measured: tests.length, reused: 0 };
  emit({ ev: "fact", kind: "per-test-map", version: 2, payload });
}

function emit(message: object): void {
  process.stdout.write(`${JSON.stringify(message)}\n`);
}

const command = process.argv[2] ?? "";
if (command === "describe") {
  emit(manifest());
} else if (command === "run") {
  run(JSON.parse(readFileSync(0, "utf8")));
} else {
  process.stderr.write(`usage: produce.mts describe|run, not \`${command}\`\n`);
  process.exit(64);
}
