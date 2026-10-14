// ABOUTME: The Go Symbols producer against the Go fixture repository: functions, methods namespaced by
// ABOUTME: their receiver type, named function literals, package namespaces, and visibility by case.
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
use Visibility::{Internal, Public};

#[test]
fn package_files_have_functions_methods_and_closures_namespaced_by_package_and_receiver() {
    let symbols: Symbols = produce("go", &fixture("go"));
    expect(
        &symbols,
        "ledger.go",
        &[
            ("ledger.init", Function, Internal, (21, 23)),
            ("ledger.NewAccount", Function, Public, (38, 42)),
            ("ledger.Account.Deposit", Method, Public, (45, 47)),
            ("ledger.Account.Statement", Method, Public, (50, 55)),
            (
                "ledger.Account.Statement.header",
                Closure,
                Internal,
                (51, 53),
            ),
            ("ledger.Account.audited", Method, Internal, (57, 59)),
            ("ledger.Balance", Function, Public, (62, 64)),
            ("ledger.scale", Function, Internal, (66, 72)),
            ("ledger.FloorArea", Function, Public, (75, 77)),
        ],
    );
}

#[test]
fn the_namespace_is_the_package_name_not_the_directory() {
    let symbols: Symbols = produce("go", &fixture("go"));
    expect(
        &symbols,
        "report/format.go",
        &[("report.Render", Function, Public, (6, 8))],
    );
    expect(
        &symbols,
        "cmd/ledger/main.go",
        &[("main.main", Function, Internal, (10, 14))],
    );
    expect(
        &symbols,
        "shapes/geometry.go",
        &[
            ("shapes.NewCircle", Function, Public, (9, 11)),
            ("shapes.Circle.Area", Method, Public, (13, 15)),
            ("shapes.Circle.scaled", Method, Internal, (17, 22)),
            ("shapes.Circle.scaled.double", Closure, Internal, (18, 20)),
            ("shapes.Square", Function, Public, (24, 26)),
        ],
    );
}

#[test]
fn test_files_hold_test_functions_and_test_doubles() {
    let symbols: Symbols = produce("go", &fixture("go"));
    let names: Vec<String> = rows(&symbols, "pricing_test.go")
        .into_iter()
        .map(|row| row.0)
        .collect();
    assert_eq!(
        names,
        [
            "ledger.MockNotifier.Notify",
            "ledger.TestTallyReportsOverdraft",
            "ledger.TestParseCentsAcceptsDigits",
            "ledger.TestParseCentsRejectsBlanks",
            "ledger.TestDiscountSnapshot",
            "ledger.TestMailNotifiesTheOwner",
        ]
    );
}

#[test]
fn generic_receivers_package_level_function_variables_and_anonymous_literals() {
    let dir = tree(&[(
        "stack.go",
        "package stack\n\n\
         type Stack[T any] struct{ items []T }\n\n\
         func (s *Stack[T]) Push(item T) { s.items = append(s.items, item) }\n\n\
         func (s Stack[T]) peek() T { return s.items[len(s.items)-1] }\n\n\
         var (\n\tdefaultLess = func(a, b int) bool { return a < b }\n)\n\n\
         func Run() {\n\tgo func() {}()\n\tvar done = func() {}\n\tdone()\n}\n",
    )]);
    let symbols: Symbols = produce("go", dir.path());
    expect(
        &symbols,
        "stack.go",
        &[
            ("stack.Stack.Push", Method, Public, (5, 5)),
            ("stack.Stack.peek", Method, Internal, (7, 7)),
            ("stack.defaultLess", Closure, Internal, (10, 10)),
            ("stack.Run", Function, Public, (13, 17)),
            ("stack.Run.done", Closure, Internal, (15, 15)),
        ],
    );
}

#[test]
fn symbols_carry_their_name_namespace_language_and_body() {
    let symbols: Symbols = produce("go", &fixture("go"));
    let deposit = symbols
        .symbols
        .iter()
        .find(|s| s.display == "ledger.Account.Deposit")
        .unwrap();
    assert_eq!(deposit.name, "Deposit");
    assert_eq!(deposit.namespace, "ledger.Account");
    assert_eq!(deposit.lang.as_str(), "go");
    let body = deposit.body.unwrap();
    assert_eq!(
        (body.line, body.col, body.end_line, body.end_col),
        (45, 40, 47, 2)
    );
}
