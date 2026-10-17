# ABOUTME: The Python harness of Cupel's metamorphic engine (ADR-097 §2): reads one relation and a batch of inputs as
# ABOUTME: JSON on standard input, evaluates the relation on each until one does not hold, and prints the outcome.
import copy
import importlib
import inspect
import json
import os
import sys


class Setup(Exception):
    """The relation cannot be evaluated at all: its function, transform, or relation is wrong."""


def describe(error):
    return f"{type(error).__name__}: {error}"


def resolve(reference):
    module_name, _, attribute = reference.partition(":")
    if not module_name or not attribute:
        raise Setup(f"`{reference}` is not `module:name`")
    try:
        module = importlib.import_module(module_name)
    except Exception as error:
        raise Setup(f"cannot import `{module_name}`: {describe(error)}") from None
    target = module
    for part in attribute.split("."):
        if not hasattr(target, part):
            raise Setup(f"`{module_name}` has no `{attribute}`")
        target = getattr(target, part)
    if not callable(target):
        raise Setup(f"`{reference}` is not callable")
    return module, target


def compiled(key, source, namespace):
    try:
        value = eval(source, dict(namespace))
    except Exception as error:
        raise Setup(f"`{key}` does not evaluate: {describe(error)}") from None
    if not callable(value):
        raise Setup(f"`{key}` is not callable")
    return value


def call(function, value, spread):
    """The function on its own copy of `value`, so a function that changes its argument changes
    nothing the other calls see."""
    value = copy.deepcopy(value)
    try:
        return ("value", function(*value) if spread else function(value))
    except Exception as error:
        return ("raised", describe(error))


def encoded(outcome):
    kind, value = outcome
    if kind == "raised":
        return {"raised": value}
    try:
        return {"value": json.loads(json.dumps(value, allow_nan=False))}
    except (TypeError, ValueError):
        return {"repr": repr(value)}


def holds(relation, source, followup):
    if relation is None:
        return source == followup
    return bool(relation(source, followup))


def violation(function, transform, relation, value, spread):
    """What is wrong with the relation on `value`, or None when it holds."""
    source = call(function, value, spread)
    transformed = call(transform, value, False)
    followup = call(function, transformed[1], spread) if transformed[0] == "value" else None
    found = {
        "transformed": encoded(transformed),
        "source": encoded(source),
        "followup": encoded(followup) if followup else None,
        "relation_raised": None,
    }
    if source[0] != "value" or followup is None or followup[0] != "value":
        return found
    try:
        if holds(relation, source[1], followup[1]):
            return None
    except Exception as error:
        found["relation_raised"] = describe(error)
    return found


def location(function):
    try:
        target = inspect.unwrap(function)
        path = inspect.getsourcefile(target)
        lines, start = inspect.getsourcelines(target)
    except (TypeError, OSError):
        return None
    if path is None:
        return None
    relative = os.path.relpath(os.path.realpath(path), os.path.realpath(os.getcwd()))
    if relative.startswith(".."):
        return None
    line = max(start, 1)
    return {
        "file": relative.replace(os.sep, "/"),
        "line": line,
        "col": None,
        "end_line": line + len(lines) - 1,
        "end_col": None,
    }


def evaluate(request):
    module, function = resolve(request["function"])
    namespace = vars(module)
    transform = compiled("transform", request["transform"], namespace)
    relation = None
    if request["relation"] is not None:
        relation = compiled("relation", request["relation"], namespace)
    spread = request["spread"]
    for index, value in enumerate(request["inputs"]):
        native = tuple(value) if spread else value
        found = violation(function, transform, relation, native, spread)
        if found is not None:
            found["index"] = index
            return {"location": location(function), "evaluated": index + 1, "violation": found}
    return {"location": location(function), "evaluated": len(request["inputs"]), "violation": None}


def main():
    request = json.load(sys.stdin)
    out = sys.stdout
    # What the code under test prints goes to standard error, so standard output holds only
    # the outcome.
    sys.stdout = sys.stderr
    try:
        response = evaluate(request)
    except Setup as error:
        response = {"error": str(error)}
    out.write(json.dumps(response))
    out.write("\n")


main()
