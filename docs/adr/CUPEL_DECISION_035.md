# CUPEL_DECISION_035: Python module resolution: projects, import roots, and what is external

**Date:** 2026-10-10 · **Status:** accepted · **Phase:** P1.7 (python)

## Context

ADR-028 fixes the `ModuleGraph` interface and Rust's rules and leaves each other language's rules to its resolver. Python resolves imports at run time against `sys.path`, which depends on how the program is started (an installed package, `python -m`, a script, pytest's rootdir insertion) and on the environment, which a static producer cannot see. Packages may be regular (`__init__.py`) or namespace (PEP 420) directories, projects use either the `src/` or the flat layout, and monorepos hold several projects (uv workspaces, path dependencies). The distribution name a manifest lists often differs from the module a program imports (`PyYAML` and `yaml`).

## Options

**Import roots.** (a) The project directory only: misses the `src/` layout. (b) For each file, the directories Python and pytest would put on `sys.path` for it: its *base directory* (the first ancestor without `__init__.py`, which is what pytest inserts for a test file and what a script's directory is), then its project's `src/` and the project directory, then every other project's roots in the repository.

**External imports.** (a) Only declared dependencies are `External`; others `Unresolved`, as for Rust. Wrong whenever a distribution's name differs from its module's, and for transitive dependencies. (b) Every absolute import that no import root contains is `External`, named by its top-level module; `Unresolved` is kept for imports that should be in the repository and are not.

**Specifiers.** One edge per statement loses which names a `from` import brings in, and a name may be a submodule (`from . import format`) that resolves to its own file. One edge per imported name, specified as the module path joined with the name (as Rust expands use trees), keeps both.

## Decision

- **Projects** are directories with `pyproject.toml`, `setup.py`, or `setup.cfg`: the nearest above each file in scope, and the `[tool.uv.workspace] members` (with `*` globs) of every project found. `RootImports.root` is the project directory; files outside every project group under `.`.
- **Import roots** of a file, in order: its base directory, its project's `src/` (when it exists) and its project directory (the repository root for files outside every project), then the `src/` and directory of every other project. Names compare case-sensitively, as Python's importer does, even on case-insensitive file systems.
- **Absolute imports** `import a.b` and `from a.b import n`: the first root containing `a` (a `.py` file or a directory) decides. Every segment but the last must be a package directory; the last may be a module file or a package. The target is `n`'s own file when `n` is a submodule (`n.py` or `n/__init__.py`), else the module's file (`b.py` or `b/__init__.py`). A path that starts in the repository but does not resolve to a file (a missing module, a namespace package with no file) is `Unresolved`. A path no root contains is `External { package: "a" }`, standard library and third-party alike.
- **Relative imports** `from .x import n` and `from .. import n` start at the importing file's directory, one level up per extra dot, and resolve the same way below it; they are `Unresolved` when they leave the repository or name nothing. `relative` is true exactly for these.
- **Specifiers** are the module as written joined with each imported name: `ledger.decide.parse_cents`, `.format`, `..pkg.*`; `import a.b as c` is `a.b`. `from __future__` imports are not edges.

## Consequences

- A typo in a third-party import (`import reqeusts`) is `External`, not `Unresolved`; `static.deps` can compare `External` packages with declared dependencies itself if it needs to.
- Files reached only through `sys.path` manipulation, `.pth` files, or installed editable packages outside the repository resolve as `External` or `Unresolved`.
- Any project in the repository can import any other without declaring it, which matches how a uv workspace or an editable install behaves.
