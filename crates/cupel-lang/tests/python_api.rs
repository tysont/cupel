// ABOUTME: The Python ApiSurface and Roots producers: public items under `__all__` and underscore rules with
// ABOUTME: normalized signatures, and symbols live by convention (main, `__main__` guards, tests, hooks).
mod support;

use cupel_core::syntax::{ApiItemKind, ApiSurface, RootReason, Roots, Symbols};
use support::{assert_every_case_is_a_test_root, fixture, items, produce, reasons, tree};

use ApiItemKind::{Constant, Function, Interface, Method, Type, Variable};
use RootReason::{EntryPoint, Export, FrameworkHook, Test};

fn item(path: &str, kind: ApiItemKind, signature: &str) -> (String, ApiItemKind, String) {
    (path.to_owned(), kind, signature.to_owned())
}

#[test]
fn a_package_with_all_exports_only_the_names_it_lists() {
    let api: ApiSurface = produce("python", &fixture("python"));
    assert_eq!(
        items(&api, "src/ledger/__init__.py"),
        [
            item("ledger.MAX_ENTRIES", Constant, "int"),
            item("ledger.default_currency", Variable, ""),
            item("ledger.balance", Function, "(_: list[int]) -> int"),
        ]
    );
}

#[test]
fn classes_protocols_and_their_public_methods_have_paths_and_signatures() {
    let api: ApiSurface = produce("python", &fixture("python"));
    assert_eq!(
        items(&api, "src/ledger/account.py"),
        [
            item("ledger.account.Audited", Interface, "(Protocol)"),
            item(
                "ledger.account.Audited.audit_trail",
                Method,
                "(self) -> list[str]"
            ),
            item("ledger.account.Audited.is_clean", Method, "(self) -> bool"),
            item("ledger.account.Account", Type, ""),
            item("ledger.account.Account.__init__", Method, "(self, _: str)"),
            item("ledger.account.Account.opening", Method, "(_: str)"),
            item(
                "ledger.account.Account.deposit",
                Method,
                "(self, _: int) -> None"
            ),
            item("ledger.account.Account.statement", Method, "(self) -> str"),
        ]
    );
}

#[test]
fn without_all_every_module_level_name_without_an_underscore_is_public() {
    let dir = tree(&[(
        "pkg/shapes.py",
        "from abc import ABC\nfrom dataclasses import dataclass\n\n\
         type Meters = float\n\
         ORIGIN: tuple[int, int] = (0, 0)\n\
         registry = {}\n\
         _cache = {}\n\
         scale = lambda shape, factor=2: shape\n\n\
         class Shape(ABC):\n    def area(self) -> float:\n        pass\n\n\
         @dataclass\nclass Square[T](Shape):\n    side: T\n\n    \
         @property\n    def area(self) -> float:\n        return self.side ** 2\n\n    \
         def _grow(self):\n        pass\n\n\
         class _Draft:\n    def area(self):\n        pass\n\n\
         def build(*sides: int, unit: str = \"m\", **options) -> Square:\n    pass\n\n\
         def _helper():\n    pass\n",
    )]);
    let api: ApiSurface = produce("python", dir.path());
    assert_eq!(
        items(&api, "pkg/shapes.py"),
        [
            item("pkg.shapes.Meters", Type, "float"),
            item("pkg.shapes.ORIGIN", Constant, "tuple[int, int]"),
            item("pkg.shapes.registry", Variable, ""),
            item("pkg.shapes.scale", Function, "_, _ = 2"),
            item("pkg.shapes.Shape", Interface, "(ABC)"),
            item("pkg.shapes.Shape.area", Method, "(self) -> float"),
            item("pkg.shapes.Square", Type, "[T] (Shape)"),
            item("pkg.shapes.Square.area", Method, "(self) -> float"),
            item(
                "pkg.shapes.build",
                Function,
                "(*_: int, _: str = \"m\", ** _) -> Square"
            ),
        ]
    );
}

#[test]
fn with_all_unlisted_classes_and_their_methods_are_not_public() {
    let dir = tree(&[(
        "mod.py",
        "__all__ = [\"Listed\"]\n\n\
         class Listed:\n    def run(self):\n        pass\n\n\
         class Unlisted:\n    def run(self):\n        pass\n\n\
         LIMIT = 3\n",
    )]);
    let api: ApiSurface = produce("python", dir.path());
    assert_eq!(
        items(&api, "mod.py"),
        [
            item("mod.Listed", Type, ""),
            item("mod.Listed.run", Method, "(self)")
        ]
    );
}

/// Signatures are token spans joined by `cupel_lang::api`'s spacing rule; a string literal is one
/// token, so its quotes stay attached (`""` reads `""`).
#[test]
fn signatures_ignore_layout_and_parameter_names_and_see_removed_parameters() {
    let compact = tree(&[(
        "m.py",
        "def pay(cents: int, memo: str = \"\") -> None:\n    pass\n",
    )]);
    let spread = tree(&[(
        "m.py",
        "def pay(\n    amount: int,  # in cents\n    note: str = \"\",\n) -> None:\n    pass\n",
    )]);
    let removed = tree(&[("m.py", "def pay(cents: int) -> None:\n    pass\n")]);
    let compact: ApiSurface = produce("python", compact.path());
    let spread: ApiSurface = produce("python", spread.path());
    let removed: ApiSurface = produce("python", removed.path());
    assert_eq!(
        compact.items[0].signature,
        "(_: int, _: str = \"\") -> None"
    );
    assert_eq!(spread.items[0].signature, compact.items[0].signature);
    assert_eq!(removed.items[0].path, compact.items[0].path);
    assert_ne!(removed.items[0].signature, compact.items[0].signature);
}

#[test]
fn main_guards_exports_tests_fixtures_and_dunders_are_roots() {
    let root = fixture("python");
    let roots: Roots = produce("python", &root);
    let symbols: Symbols = produce("python", &root);
    let of = |display: &str| reasons(&roots, &symbols, display);
    assert_eq!(of("ledger.cli.main"), [EntryPoint, Export]);
    assert_eq!(of("ledger.cli.run"), [EntryPoint, Export]);
    assert_eq!(of("ledger.balance"), [Export]);
    assert!(of("ledger.audit_limit").is_empty());
    assert!(of("ledger.account.Account.statement.line").is_empty());
    assert_eq!(
        of("ledger.account.Account.__init__"),
        [Export, FrameworkHook]
    );
    assert_eq!(
        of("tests.test_ledger.test_deposit_adds_to_the_balance"),
        [Export, Test]
    );
    assert_eq!(of("tests.conftest.account"), [Export, FrameworkHook]);
    assert_eq!(
        of("tests.test_pricing.PricingTest.setUp"),
        [Export, FrameworkHook]
    );
}

#[test]
fn every_test_case_in_the_fixture_is_a_test_root() {
    assert_every_case_is_a_test_root("python", 29);
}

#[test]
fn a_decorated_command_called_through_sys_exit_in_the_guard_is_an_entry_point() {
    let dir = tree(&[(
        "tool.py",
        "import sys\n\n\
         @click.command()\ndef cli():\n    pass\n\n\
         def _unused():\n    pass\n\n\
         def pytest_configure(config):\n    pass\n\n\
         if __name__ == \"__main__\":\n    sys.exit(cli())\n",
    )]);
    let roots: Roots = produce("python", dir.path());
    let symbols: Symbols = produce("python", dir.path());
    assert_eq!(reasons(&roots, &symbols, "tool.cli"), [EntryPoint, Export]);
    assert!(reasons(&roots, &symbols, "tool._unused").is_empty());
    assert_eq!(
        reasons(&roots, &symbols, "tool.pytest_configure"),
        [Export, FrameworkHook]
    );
}

#[test]
fn a_function_the_module_level_code_calls_is_an_entry_point() {
    // A setup script: setuptools runs its body, which calls its own functions, directly, as an
    // argument, or as a keyword argument's value; nothing imports it.
    let dir = tree(&[(
        "setup.py",
        "from setuptools import setup\n\n\
         def get_version():\n    return '1'\n\n\
         def get_long_description():\n    return ''\n\n\
         def configure():\n    pass\n\n\
         def banner():\n    return ''\n\n\
         def unused():\n    pass\n\n\
         configure()\n\
         TITLE = banner()\n\
         setup(\n    get_long_description(),\n    version=get_version(),\n)\n",
    )]);
    let roots: Roots = produce("python", dir.path());
    let symbols: Symbols = produce("python", dir.path());
    for called in [
        "setup.get_version",
        "setup.get_long_description",
        "setup.configure",
        "setup.banner",
    ] {
        assert_eq!(
            reasons(&roots, &symbols, called),
            [EntryPoint, Export],
            "{called}"
        );
    }
    assert_eq!(reasons(&roots, &symbols, "setup.unused"), [Export]);
}
