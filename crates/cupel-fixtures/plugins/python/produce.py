# ABOUTME: The Python reference producer plugin (ADR-092): a `per-test-map` fact for JavaScript and TypeScript read
# ABOUTME: from a committed LCOV file with one `TN:` section per test; speaks the plugin protocol with the standard library.
import json
import os
import sys

PRODUCER = "plugin.ref-python"
DEFAULT_LCOV = "coverage/per-test.lcov"


def manifest():
    return {
        "kind": "producer",
        "id": PRODUCER,
        "version": 1,
        "provides": {"kind": "per-test-map", "version": 2},
        "languages": ["javascript", "typescript"],
        "needs": [],
        "wants": [],
        "cost": "quick",
        "stability": "experimental",
        "summary": "The lines each test executes, from an LCOV file with a TN: section per test.",
    }


def per_test(text):
    """Each `TN:` test's covered lines, by file: `DA:<line>,<hits>` records with hits above zero."""
    tests = {}
    test = file = None
    for record in text.splitlines():
        key, _, value = record.partition(":")
        if key == "TN":
            test = value
            tests.setdefault(test, {})
        elif key == "SF":
            file = value
        elif key == "DA" and test is not None and file is not None:
            line, hits = value.split(",")[:2]
            if int(hits) > 0:
                tests[test].setdefault(file, set()).add(int(line))
    return [
        {
            "id": test,
            "file": None,
            "covered": [
                {"file": file, "lines": sorted(lines)} for file, lines in sorted(files.items())
            ],
        }
        for test, files in sorted(tests.items())
    ]


def run(request):
    path = os.path.join(request["root"], request["config"].get("per_test_lcov", DEFAULT_LCOV))
    try:
        with open(path, encoding="utf-8") as source:
            tests = per_test(source.read())
    except OSError as error:
        print(f"cannot read the per-test LCOV file: {error}", file=sys.stderr)
        sys.exit(1)
    payload = {"runner": "tool.vitest", "tests": tests, "measured": len(tests), "reused": 0}
    emit({"ev": "fact", "kind": "per-test-map", "version": 2, "payload": payload})


def emit(message):
    print(json.dumps(message, ensure_ascii=False))


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "describe":
        emit(manifest())
    elif command == "run":
        run(json.load(sys.stdin))
    else:
        print(f"usage: produce.py describe|run, not `{command}`", file=sys.stderr)
        sys.exit(64)


main()
