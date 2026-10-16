# ABOUTME: The Python reference check plugin (ADR-092): reports each file in scope that no test in the per-test
# ABOUTME: map executes, as `ref.untested-file`; speaks the plugin protocol (design §8.6) with the standard library.
import json
import sys

CHECK = "ref.untested-python"
KIND = "ref.untested-file"


def manifest():
    return {
        "kind": "check",
        "id": CHECK,
        "version": 1,
        "tier": 2,
        "cost": "quick",
        "needs": ["per-test-map"],
        "wants": [],
        "findings": [
            {
                "kind": KIND,
                "action": "write_test",
                "summary": "A file in scope that no test executes.",
                "evidence": {
                    "type": "object",
                    "properties": {"file": {"type": "string"}},
                    "required": ["file"],
                    "additionalProperties": False,
                },
            }
        ],
        "metrics": [
            {"name": "untested", "summary": "Files in scope no test executes.", "per": "scope"}
        ],
        "stability": "experimental",
        "summary": "Files in scope that no test executes, from the per-test map.",
    }


def finding_id(file):
    """`fnd_` and six hex digits of 32-bit FNV-1a over the check, kind, and file joined by NUL."""
    digest = 0x811C9DC5
    for byte in f"{CHECK}\0{KIND}\0{file}".encode():
        digest = ((digest ^ byte) * 0x01000193) & 0xFFFFFFFF
    return f"fnd_{digest:08x}"[:10]


def finding(file):
    identity = finding_id(file)
    return {
        "ev": "finding",
        "id": identity,
        "check": CHECK,
        "kind": KIND,
        "severity": "warn",
        "confidence": 1.0,
        "tier": 2,
        "symbol": None,
        "location": {"file": file, "line": 1, "col": None, "end_line": 1, "end_col": None},
        "title": f"No test executes {file}",
        "detail": None,
        "evidence": {"file": file},
        "action": {
            "kind": "write_test",
            "priority": 1,
            "summary": f"Write a test that executes {file}.",
            "hints": {},
        },
        "repro": f"cupel check --only {CHECK}",
        "suppress": f"cupel suppress {identity} --reason '…'",
        "new_since_baseline": False,
        "suppressed": False,
    }


def run(request):
    with open(request["facts"]["per-test-map"], encoding="utf-8") as source:
        per_test = json.load(source)
    covered = {
        lines["file"]
        for test in per_test["tests"]
        for lines in test["covered"]
        if lines["lines"]
    }
    files = request["scope"]["files"]
    untested = [file for file in files if file not in covered]
    for file in untested:
        emit(finding(file))
    tested = len(files) - len(untested)
    emit(
        {
            "ev": "result",
            "status": "fail" if untested else "pass",
            "score": {
                "value": tested / len(files) if files else 1.0,
                "unit": "tested_files",
                "n": len(files),
            },
            "metrics": {"untested": len(untested)},
            "notes": [],
        }
    )


def emit(message):
    print(json.dumps(message, ensure_ascii=False))


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "describe":
        emit(manifest())
    elif command == "plan":
        files = json.load(sys.stdin)["scope"]["files"]
        emit({"targets": len(files), "est_ms": len(files)})
    elif command == "run":
        run(json.load(sys.stdin))
    else:
        print(f"usage: check.py describe|plan|run, not `{command}`", file=sys.stderr)
        sys.exit(64)


main()
