# ABOUTME: The Python harness of Cupel's metamorphic engine (ADR-097 §2): reads one relation and a batch of inputs as
# ABOUTME: JSON on standard input, evaluates the relation on each until one does not hold, and prints the outcome.
from __future__ import annotations

import copy
import importlib
import inspect
import json
import os
import sys
from collections.abc import Callable
from types import ModuleType
from typing import Any


class Setup(Exception):
    """The relation cannot be evaluated at all: its function, transform, or relation is wrong."""


def describe(error: BaseException) -> str:
    return f"{type(error).__name__}: {error}"


def resolve(reference: str) -> tuple[ModuleType, Callable[..., Any]]:
    module_name, _, attribute = reference.partition(":")
    if not module_name or not attribute:
        raise Setup(f"`{reference}` is not `module:name`")
    try:
        module = importlib.import_module(module_name)
    except Exception as error:  # noqa: BLE001 - the project's code may raise anything
        raise Setup(f"cannot import `{module_name}`: {describe(error)}") from None
    target: Any = module
    for part in attribute.split("."):
        if not hasattr(target, part):
            raise Setup(f"`{module_name}` has no `{attribute}`")
        target = getattr(target, part)
    if not callable(target):
        raise Setup(f"`{reference}` is not callable")
    return module, target


def compiled(key: str, source: str, namespace: dict[str, Any]) -> Callable[..., Any]:
    try:
        value = eval(source, dict(namespace))
    except Exception as error:  # noqa: BLE001 - the project's code may raise anything
        raise Setup(f"`{key}` does not evaluate: {describe(error)}") from None
    if not callable(value):
        raise Setup(f"`{key}` is not callable")
    function: Callable[..., Any] = value
    return function


def call(function: Callable[..., Any], value: Any, spread: bool) -> tuple[str, Any]:
    """The function on its own copy of `value`, so a function that changes its argument changes
    nothing the other calls see: ("value", what it returned) or ("raised", what it raised)."""
    value = copy.deepcopy(value)
    try:
        return ("value", function(*value) if spread else function(value))
    except Exception as error:  # noqa: BLE001 - the project's code may raise anything
        return ("raised", describe(error))


def encoded(outcome: tuple[str, Any]) -> dict[str, Any]:
    kind, value = outcome
    if kind == "raised":
        return {"raised": value}
    try:
        return {"value": json.loads(json.dumps(value, allow_nan=False))}
    except (TypeError, ValueError):
        return {"repr": repr(value)}


def holds(relation: Callable[..., Any] | None, source: Any, followup: Any) -> bool:
    if relation is None:
        return bool(source == followup)
    return bool(relation(source, followup))


def violation(
    function: Callable[..., Any],
    transform: Callable[..., Any],
    relation: Callable[..., Any] | None,
    value: Any,
    spread: bool,
) -> dict[str, Any] | None:
    """What is wrong with the relation on `value`, or None when it holds."""
    source = call(function, value, spread)
    transformed = call(transform, value, False)
    followup = call(function, transformed[1], spread) if transformed[0] == "value" else None
    found: dict[str, Any] = {
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
    except Exception as error:  # noqa: BLE001 - the project's code may raise anything
        found["relation_raised"] = describe(error)
    return found


def location(function: Callable[..., Any]) -> dict[str, Any] | None:
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


def evaluate(request: dict[str, Any]) -> dict[str, Any]:
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


def main() -> None:
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
