# ABOUTME: Cupel's PIT producer plugin (ADR-110): `mutation-outcomes` for Java from PIT's XML report, PIT run on a copy
# ABOUTME: of the project in the plugin's scratch directory; each PIT mutation is placed on Cupel's mutation sites.
"""Cupel's PIT producer plugin.

Speaks Cupel's plugin protocol (`describe`, `run`) with the standard library; name it in
`cupel.toml`:

    [plugins]
    pit = ["python3", "plugins/pit/cupel_produce_pit.py"]

    [lang.java]
    mutator = "plugin.pit"            # select it over the native engine
    pit = "mvn -B -q ... mutationCoverage"   # optional: the command that runs PIT
    pit_report = "target/pit-reports/mutations.xml"   # optional: where it writes its XML

The command runs in a copy of the project under the run's scratch directory, so nothing is
written into the tree. With `pit_report_ready = true` the report already in the tree is read
instead and PIT is not run.
"""
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import xml.etree.ElementTree as ElementTree

PRODUCER = "plugin.pit"

# The facts mutations are placed on: functions, their structural hashes, and Cupel's sites.
NEEDS = ["symbols", "norm-tree", "mutation-sites"]

# The command that runs PIT, and where its XML report lands, per build tool. Maven needs the
# PIT plugin (with its JUnit 5 plugin) in the `pom.xml`; Gradle needs the `info.solidsoft.pitest`
# plugin configured with `outputFormats = ['XML']` and `timestampedReports = false`.
MAVEN = (
    "mvn -B -q --no-transfer-progress test-compile org.pitest:pitest-maven:mutationCoverage "
    "-DoutputFormats=XML -DtimestampedReports=false -DfullMutationMatrix=true",
    "target/pit-reports/mutations.xml",
)
GRADLE = ("gradle pitest --console=plain", "build/reports/pitest/mutations.xml")

# Directories never copied: version control, Cupel's own, build output, and tool state.
SKIPPED_DIRS = {".git", ".cupel", "target", "build", ".gradle", "node_modules"}

# Each PIT mutator's Cupel operators, the first a site on the mutation's line has wins, and
# whether PIT's replacement is the one Cupel's tables name for that operator.
MUTATORS = {
    "ConditionalsBoundaryMutator": (["relational"], True),
    "NegateConditionalsMutator": (["equality", "relational", "negation"], False),
    "IncrementsMutator": (["arith-add-sub"], True),
    "InvertNegsMutator": (["arith-add-sub"], False),
    "VoidMethodCallMutator": (["statement-deletion"], True),
    "EmptyObjectReturnValsMutator": (["return-value"], False),
    "NullReturnValsMutator": (["return-value"], True),
    "PrimitiveReturnsMutator": (["return-value"], False),
    "BooleanTrueReturnValsMutator": (["return-value"], False),
    "BooleanFalseReturnValsMutator": (["return-value"], False),
    "RemoveConditionalMutator_EQUAL_IF": (["negation"], False),
    "RemoveConditionalMutator_EQUAL_ELSE": (["negation"], False),
    "RemoveConditionalMutator_ORDER_IF": (["negation"], False),
    "RemoveConditionalMutator_ORDER_ELSE": (["negation"], False),
}

# PIT's statuses as Cupel's outcomes.
OUTCOMES = {
    "KILLED": "killed",
    "SURVIVED": "survived",
    "NO_COVERAGE": "uncovered",
    "TIMED_OUT": "timeout",
    "MEMORY_ERROR": "unviable",
    "RUN_ERROR": "unviable",
    "NON_VIABLE": "unviable",
}


def manifest():
    return {
        "kind": "producer",
        "id": PRODUCER,
        "version": 1,
        "provides": {"kind": "mutation-outcomes", "version": 2},
        "languages": ["java"],
        "needs": NEEDS,
        "wants": [],
        "cost": "standard",
        "stability": "experimental",
        "summary": "Mutation outcomes for Java from PIT, placed on Cupel's mutation sites.",
    }


def math_operators(description):
    """The operator a MathMutator's description names: addition and subtraction, or the rest."""
    if re.search(r"addition|subtraction", description):
        return ["arith-add-sub"]
    return ["arith-mul-div"]


def operators_of(mutator, description):
    """The Cupel operators PIT's `mutator` maps to, and whether its replacement is Cupel's."""
    name = mutator.rsplit(".", 1)[-1]
    if name == "MathMutator":
        return math_operators(description), True
    return MUTATORS.get(name, ([], False))


def runner_id(name):
    """A PIT test name as the Maven runner's ID, `<class>#<method>`."""
    found = re.search(r"\[class:([^\]]+)\].*\[method:([^\](]+)", name)
    if found:
        return f"{found.group(1)}#{found.group(2)}"
    found = re.match(r"^(.*)\.([^.(]+)\((.*)\)$", name)
    if found:
        return f"{found.group(3) or found.group(1)}#{found.group(2)}"
    return name


def named_tests(element, tag):
    text = element.findtext(tag) or ""
    return [runner_id(name) for name in text.split("|") if name]


def read_report(text):
    """Every mutation in PIT's XML report, as plain values."""
    mutations = []
    for element in ElementTree.fromstring(text).iter("mutation"):
        mutations.append(
            {
                "status": element.get("status", ""),
                "source": element.findtext("sourceFile") or "",
                "class": element.findtext("mutatedClass") or "",
                "method": element.findtext("mutatedMethod") or "",
                "signature": element.findtext("methodDescription") or "",
                "line": int(element.findtext("lineNumber") or 0),
                "mutator": element.findtext("mutator") or "",
                "indexes": [index.text for index in element.iter("index")],
                "description": element.findtext("description") or "",
                "killing": named_tests(element, "killingTests"),
                "succeeding": named_tests(element, "succeedingTests"),
            }
        )
    return mutations


def file_of(mutation, files):
    """The file in `files` the mutated class's source is: its package path and source file name."""
    package = mutation["class"].rsplit(".", 1)[0] if "." in mutation["class"] else ""
    suffix = "/".join(filter(None, [package.replace(".", "/"), mutation["source"]]))
    for file in files:
        if file == suffix or file.endswith("/" + suffix):
            return file
    return None


class Facts:
    """The facts the plugin needs, indexed by file, and the root their files are below."""

    def __init__(self, root, symbols, norm_tree, sites):
        self.root = root
        self.symbols = symbols["symbols"]
        self.hashes = {key(entry["symbol"]): entry["hash"] for entry in norm_tree["symbols"]}
        self.sites = {}
        for entry in sites["symbols"]:
            for site in entry["sites"]:
                self.sites.setdefault(entry["symbol"]["file"], []).append((entry["symbol"], site))

    def files(self):
        return sorted({symbol["file"] for symbol in self.symbols})

    def sites_at(self, file, line, operators):
        """The sites on `line` of `file` with one of `operators`, in operator order, then column."""
        found = [
            (symbol, site)
            for symbol, site in self.sites.get(file, [])
            if site["span"]["line"] == line and site["operator"] in operators
        ]
        return sorted(found, key=lambda pair: (operators.index(pair[1]["operator"]), pair[1]["span"]["col"]))

    def symbol_at(self, file, line):
        """The innermost symbol of `file` whose span holds `line`."""
        holding = [
            symbol
            for symbol in self.symbols
            if symbol["file"] == file and symbol["span"]["line"] <= line <= symbol["span"]["end_line"]
        ]
        if not holding:
            return None
        return min(holding, key=lambda symbol: symbol["span"]["end_line"] - symbol["span"]["line"])


def key(symbol):
    return (symbol["file"], symbol["line"], symbol["col"])


def symbol_key(symbol):
    return {"file": symbol["file"], "line": symbol["span"]["line"], "col": symbol["span"]["col"]}


def fallback_hash(mutation):
    """A site hash for a mutation no Cupel site matches, from what identifies it in PIT."""
    parts = [mutation["class"], mutation["method"], mutation["signature"], mutation["mutator"]]
    digest = hashlib.blake2b("/".join(parts + mutation["indexes"]).encode(), digest_size=8)
    return digest.hexdigest()


def described(mutation):
    """PIT's description of a mutation without the method it names (`for a/B::c`)."""
    return re.sub(r" for \S+::\S+$", "", mutation["description"])


def line_text(root, file, line):
    """Line `line` of `file` below `root`, as written; empty when it cannot be read."""
    try:
        with open(os.path.join(root, file), encoding="utf-8") as source:
            lines = source.read().splitlines()
    except OSError:
        return ""
    return lines[line - 1] if 0 < line <= len(lines) else ""


def placed(mutation, file, facts, taken):
    """Where `mutation` sits among Cupel's sites: its symbol key, site, operator, and texts; the
    k-th PIT mutation of one mutator on a line takes the k-th matching site."""
    operators, cupel_replacement = operators_of(mutation["mutator"], mutation["description"])
    slot = (file, mutation["line"], mutation["mutator"])
    index = taken.get(slot, 0)
    taken[slot] = index + 1
    candidates = facts.sites_at(file, mutation["line"], operators)
    if index < len(candidates):
        symbol, site = candidates[index]
        to = site["to"][0] if cupel_replacement and site["to"] else described(mutation)
        return symbol, site["site_hash"], site["operator"], site["from"], to, site["span"]
    symbol = facts.symbol_at(file, mutation["line"])
    if symbol is None or not operators:
        return None
    line = mutation["line"]
    text = line_text(facts.root, file, line)
    source = text.strip()
    col = len(text) - len(text.lstrip()) + 1
    span = {"line": line, "col": col, "end_line": line, "end_col": col + len(source)}
    return symbol_key(symbol), fallback_hash(mutation), operators[0], source, described(mutation), span


def mutant(mutation, file, facts, taken):
    """The `Mutant` a PIT mutation is, or None when it falls outside every symbol."""
    where = placed(mutation, file, facts, taken)
    if where is None:
        return None
    symbol, site_hash, operator, source, to, span = where
    outcome = OUTCOMES.get(mutation["status"], "unviable")
    tests = mutation["killing"] + mutation["succeeding"]
    return {
        "symbol": symbol,
        "symbol_hash": facts.hashes.get(key(symbol)),
        "site_hash": site_hash,
        "operator": operator,
        "from": source,
        "to": to,
        "span": span,
        "outcome": outcome,
        "tests": [] if outcome == "uncovered" else tests,
        "killing_tests": mutation["killing"],
        "duration_ms": None,
        "reused": False,
        "equivalent": None,
        "cache_key": None,
        "strategy": None,
        "fallback": None,
    }


def outcomes(mutations, facts, scope, depth):
    """The `mutation-outcomes` payload of `mutations` whose files are in `scope`."""
    files = facts.files()
    taken = {}
    mutants = []
    for mutation in mutations:
        file = file_of(mutation, files)
        if file is None or (scope and file not in scope):
            continue
        found = mutant(mutation, file, facts, taken)
        if found is not None:
            mutants.append(found)
    mutants.sort(key=lambda m: (m["symbol"]["file"], m["span"]["line"], m["span"]["col"], m["to"]))
    return {
        "engine": PRODUCER,
        "runner": PRODUCER,
        "depth": depth or "standard",
        "per_test": True,
        "mutants": mutants,
    }


def build_tool(root):
    """PIT's command and report for the build in `root`: Maven's, else Gradle's."""
    return MAVEN if os.path.isfile(os.path.join(root, "pom.xml")) else GRADLE


def copy_tree(source, target):
    shutil.copytree(
        source,
        target,
        symlinks=False,
        ignore=lambda _, names: [name for name in names if name in SKIPPED_DIRS],
    )


def run_pit(request, command):
    """Runs `command` in a copy of the project in the scratch directory; that copy's root."""
    work = os.path.join(request["scratch"], "tree")
    shutil.rmtree(work, ignore_errors=True)
    copy_tree(request["root"], work)
    budget = request.get("budget_ms")
    try:
        ran = subprocess.run(
            ["sh", "-c", command],
            cwd=work,
            capture_output=True,
            text=True,
            timeout=budget / 1000 if budget else None,
        )
    except subprocess.TimeoutExpired:
        fail(f"`{command}` did not finish within the run's budget")
    if ran.returncode != 0:
        tail = "\n".join((ran.stdout + ran.stderr).strip().splitlines()[-20:])
        fail(f"`{command}` exited with {ran.returncode}:\n{tail}")
    return work


def report_text(request):
    """PIT's XML report: the one in the tree when the configuration says it is ready, else the
    one a fresh run of PIT writes in its copy of the tree, which is removed once read."""
    config = request.get("config") or {}
    command, report = build_tool(request["root"])
    command = config.get("pit", command)
    report = config.get("pit_report", report)
    if config.get("pit_report_ready"):
        return read_text(request["root"], report)
    work = run_pit(request, command)
    try:
        return read_text(work, report)
    finally:
        shutil.rmtree(work, ignore_errors=True)


def read_text(root, report):
    try:
        with open(os.path.join(root, report), encoding="utf-8") as source:
            return source.read()
    except OSError as error:
        fail(f"no PIT report at {report}: {error}")


def load(path):
    with open(path, encoding="utf-8") as source:
        return json.load(source)


def run(request):
    paths = request.get("facts") or {}
    if not all(kind in paths for kind in NEEDS):
        # Without the facts to place mutations on there is nothing to report, so PIT does not run.
        none = {"symbols": []}
        facts = Facts(request["root"], none, none, none)
        emit_outcomes(outcomes([], facts, set(), request.get("depth")))
        return
    facts = Facts(
        request["root"],
        load(paths["symbols"]),
        load(paths["norm-tree"]),
        load(paths["mutation-sites"]),
    )
    mutations = read_report(report_text(request))
    scope = set(request.get("scope", {}).get("files") or [])
    emit_outcomes(outcomes(mutations, facts, scope, request.get("depth")))


def emit_outcomes(payload):
    emit({"ev": "fact", "kind": "mutation-outcomes", "version": 2, "payload": payload})


def fail(message):
    print(message, file=sys.stderr)
    sys.exit(1)


def emit(message):
    print(json.dumps(message, ensure_ascii=False))


def main():
    command = sys.argv[1] if len(sys.argv) > 1 else ""
    if command == "describe":
        emit(manifest())
    elif command == "run":
        run(json.load(sys.stdin))
    else:
        fail(f"usage: cupel_produce_pit.py describe|run, not `{command}`")


if __name__ == "__main__":
    main()
