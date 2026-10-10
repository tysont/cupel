// ABOUTME: The Python Symbols producer against the Python fixture repository: functions, methods, nested
// ABOUTME: functions, and lambdas bound to names, with namespace, `_`/`__`/`__all__` visibility, and lines.
mod support;

use cupel_core::syntax::{SymbolKind, Symbols, Visibility};
use support::{fixture, lines, produce, tree};

type Row = (&'static str, SymbolKind, Visibility, (u32, u32));

fn rows(symbols: &Symbols, file: &str) -> Vec<(String, SymbolKind, Visibility, (u32, u32))> {
    symbols
        .symbols
        .iter()
        .filter(|symbol| symbol.file == file)
        .map(|s| (s.display.clone(), s.kind, s.visibility, lines(s.span)))
        .collect()
}

fn expect(symbols: &Symbols, file: &str, expected: &[Row]) {
    let expected: Vec<(String, SymbolKind, Visibility, (u32, u32))> = expected
        .iter()
        .map(|(d, k, v, l)| ((*d).to_owned(), *k, *v, *l))
        .collect();
    assert_eq!(rows(symbols, file), expected, "{file}");
}

use SymbolKind::{Closure, Function, Method};
use Visibility::{Internal, Private, Public};

#[test]
fn a_package_init_with_all_exports_listed_names_and_keeps_the_rest_internal() {
    let symbols: Symbols = produce("python", &fixture("python"));
    expect(
        &symbols,
        "src/ledger/__init__.py",
        &[
            ("ledger.balance", Function, Public, (13, 14)),
            ("ledger.audit_limit", Function, Internal, (17, 18)),
            ("ledger._scale", Function, Internal, (21, 22)),
        ],
    );
}

#[test]
fn class_methods_static_methods_nested_functions_and_lambdas() {
    let symbols: Symbols = produce("python", &fixture("python"));
    expect(
        &symbols,
        "src/ledger/account.py",
        &[
            ("ledger.account.Audited.audit_trail", Method, Public, (7, 7)),
            ("ledger.account.Audited.is_clean", Method, Public, (9, 10)),
            ("ledger.account.Account.__init__", Method, Public, (16, 18)),
            ("ledger.account.Account.opening", Method, Public, (21, 22)),
            ("ledger.account.Account.deposit", Method, Public, (24, 25)),
            ("ledger.account.Account.statement", Method, Public, (27, 32)),
            (
                "ledger.account.Account.statement.line",
                Function,
                Private,
                (28, 29),
            ),
            (
                "ledger.account.Account.statement.header",
                Closure,
                Private,
                (31, 31),
            ),
            ("ledger.account.Account._audit", Method, Internal, (34, 35)),
            ("ledger.account.Account.__check", Method, Private, (37, 38)),
        ],
    );
}

#[test]
fn modules_take_their_namespace_from_their_path_below_src() {
    let symbols: Symbols = produce("python", &fixture("python"));
    expect(
        &symbols,
        "src/ledger/report/format.py",
        &[
            ("ledger.report.format.render", Function, Public, (4, 5)),
            ("ledger.report.format.publish", Function, Public, (8, 9)),
        ],
    );
    expect(
        &symbols,
        "libs/units/src/units/__init__.py",
        &[("units.cents", Function, Public, (1, 2))],
    );
    expect(
        &symbols,
        "tests/conftest.py",
        &[("tests.conftest.account", Function, Public, (7, 8))],
    );
}

#[test]
fn without_all_every_name_follows_its_underscores() {
    let dir = tree(&[(
        "pkg/mod.py",
        "def shown():\n    pass\n\n\n@cache\ndef _hidden():\n    pass\n\n\n\
         twice = lambda x: x * 2\n\n\n\
         class Box:\n    class Lid:\n        def __mangled(self):\n            pass\n",
    )]);
    let symbols: Symbols = produce("python", dir.path());
    expect(
        &symbols,
        "pkg/mod.py",
        &[
            ("pkg.mod.shown", Function, Public, (1, 2)),
            ("pkg.mod._hidden", Function, Internal, (6, 7)),
            ("pkg.mod.twice", Closure, Public, (10, 10)),
            ("pkg.mod.Box.Lid.__mangled", Method, Private, (15, 16)),
        ],
    );
}

#[test]
fn a_tuple_all_exports_decorated_functions_and_lambdas_it_lists() {
    let dir = tree(&[(
        "flat.py",
        "__all__ = ('listed', 'double')\n\n\
         @register\ndef listed():\n    pass\n\n\
         def unlisted():\n    pass\n\n\
         double = lambda x: x * 2\n\
         triple = lambda x: x * 3\n\n\
         def __getattr__(name):\n    pass\n",
    )]);
    let symbols: Symbols = produce("python", dir.path());
    expect(
        &symbols,
        "flat.py",
        &[
            ("flat.listed", Function, Public, (4, 5)),
            ("flat.unlisted", Function, Internal, (7, 8)),
            ("flat.double", Closure, Public, (10, 10)),
            ("flat.triple", Closure, Internal, (11, 11)),
            ("flat.__getattr__", Function, Public, (13, 14)),
        ],
    );
}
