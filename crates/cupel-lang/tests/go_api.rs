// ABOUTME: The Go ApiSurface and Roots producers: exported items with receiver-qualified paths and
// ABOUTME: normalized signatures, API differences, and roots (main, exports, tests, init, TestMain).
mod support;

use cupel_core::syntax::{ApiItemKind, ApiSurface, RootReason, Roots, Symbols};
use support::{assert_every_case_is_a_test_root, fixture, items, produce, reasons, tree};

use ApiItemKind::{Constant, Function, Interface, Method, Type, Variable};

fn item(path: &str, kind: ApiItemKind, signature: &str) -> (String, ApiItemKind, String) {
    (path.to_owned(), kind, signature.to_owned())
}

#[test]
fn exported_package_items_have_paths_and_signatures() {
    let api: ApiSurface = produce("go", &fixture("go"));
    assert_eq!(
        items(&api, "ledger.go"),
        [
            item("ledger.MaxEntries", Constant, ""),
            item("ledger.Currency", Variable, ""),
            item("ledger.Account", Type, ""),
            item("ledger.Audited", Interface, ""),
            item("ledger.Audited.AuditTrail", Method, "() []string"),
            item("ledger.NewAccount", Function, "(_ string) *Account"),
            item(
                "ledger.Account.Deposit",
                Method,
                "(_ *Account) Deposit(_ int64)"
            ),
            item(
                "ledger.Account.Statement",
                Method,
                "(_ *Account) Statement() string"
            ),
            item("ledger.Balance", Function, "(_ []int64) int64"),
            item("ledger.FloorArea", Function, "(_ float64) float64"),
        ]
    );
    assert_eq!(
        items(&api, "pricing.go"),
        [
            item("ledger.Notifier", Interface, ""),
            item("ledger.Notifier.Notify", Method, "(_, _ string) error"),
            item("ledger.Mail", Function, "(_ Notifier, _ *Account) error"),
        ]
    );
    assert_eq!(
        items(&api, "decide.go"),
        [item("ledger.ErrEmpty", Variable, "")]
    );
}

#[test]
fn test_functions_are_not_api_but_exported_test_doubles_are() {
    let api: ApiSurface = produce("go", &fixture("go"));
    assert!(items(&api, "ledger_test.go").is_empty());
    assert_eq!(
        items(&api, "pricing_test.go"),
        [
            item("ledger.MockNotifier", Type, ""),
            item(
                "ledger.MockNotifier.Notify",
                Method,
                "(_ *MockNotifier) Notify(_, _ string) error"
            ),
        ]
    );
}

#[test]
fn generics_variadics_typed_constants_and_unexported_items() {
    let dir = tree(&[(
        "kit.go",
        "package kit\n\n\
         const Limit int64 = 10\n\n\
         var (\n\tDefault = New[int]()\n\thidden  = 1\n)\n\n\
         type Stack[T any] struct{ items []T }\n\n\
         type ID = string\n\n\
         type node struct{}\n\n\
         func (n *node) Visit() {}\n\n\
         func (s *Stack[T]) Push(items ...T) {}\n\n\
         func Map[T any, U any](items []T, f func(T) U) []U {\n\tvar Local int\n\t_ = Local\n\treturn nil\n}\n\n\
         func New[T any]() *Stack[T] { return &Stack[T]{} }\n\n\
         func Lookup(index map[string]int, key string) (value int, ok bool) { return }\n",
    )]);
    let api: ApiSurface = produce("go", dir.path());
    assert_eq!(
        items(&api, "kit.go"),
        [
            item("kit.Limit", Constant, "int64"),
            item("kit.Default", Variable, ""),
            item("kit.Stack", Type, "[T any]"),
            item("kit.ID", Type, ""),
            item("kit.Stack.Push", Method, "(_ *Stack[T]) Push(_ ...T)"),
            item(
                "kit.Map",
                Function,
                "[T any, U any](_ []T, _ func(T) U) []U"
            ),
            item("kit.New", Function, "[T any]() *Stack[T]"),
            item(
                "kit.Lookup",
                Function,
                "(_ map[string]int, _ string) (_ int, _ bool)"
            ),
        ]
    );
}

#[test]
fn signatures_ignore_layout_and_names_and_removing_a_parameter_changes_them() {
    let compact = tree(&[(
        "a.go",
        "package a\n\nfunc Pay(cents int64, memo string) error { return nil }\n",
    )]);
    let spread = tree(&[(
        "a.go",
        "package a\n\nfunc Pay(\n\tamount int64, // in cents\n\tnote string,\n) error {\n\treturn nil\n}\n",
    )]);
    let removed = tree(&[(
        "a.go",
        "package a\n\nfunc Pay(cents int64) error { return nil }\n",
    )]);
    let compact: ApiSurface = produce("go", compact.path());
    let spread: ApiSurface = produce("go", spread.path());
    let removed: ApiSurface = produce("go", removed.path());
    assert_eq!(compact.items[0].signature, "(_ int64, _ string) error");
    assert_eq!(spread.items[0].signature, compact.items[0].signature);
    assert_eq!(removed.items[0].path, compact.items[0].path);
    assert_eq!(removed.items[0].signature, "(_ int64) error");
}

#[test]
fn main_init_test_main_exports_and_tests_are_roots() {
    let root = fixture("go");
    let roots: Roots = produce("go", &root);
    let symbols: Symbols = produce("go", &root);
    assert_eq!(
        reasons(&roots, &symbols, "main.main"),
        [RootReason::EntryPoint]
    );
    assert_eq!(
        reasons(&roots, &symbols, "ledger.init"),
        [RootReason::FrameworkHook]
    );
    assert_eq!(
        reasons(&roots, &symbols, "ledger.TestMain"),
        [RootReason::Export, RootReason::FrameworkHook]
    );
    assert_eq!(
        reasons(&roots, &symbols, "ledger.Balance"),
        [RootReason::Export]
    );
    assert!(reasons(&roots, &symbols, "ledger.scale").is_empty());
    assert_eq!(
        reasons(&roots, &symbols, "ledger.TestClassify"),
        [RootReason::Export, RootReason::Test]
    );
}

#[test]
fn every_test_case_in_the_fixture_is_a_test_root() {
    assert_every_case_is_a_test_root("go", 23);
}

#[test]
fn benchmarks_examples_and_fuzz_targets_are_hooks() {
    let dir = tree(&[(
        "a_test.go",
        "package a\n\nimport \"testing\"\n\n\
         func BenchmarkPay(b *testing.B) {}\n\n\
         func ExamplePay() {}\n\n\
         func FuzzPay(f *testing.F) {}\n\n\
         func Benchmarking() {}\n",
    )]);
    let roots: Roots = produce("go", dir.path());
    let symbols: Symbols = produce("go", dir.path());
    for hook in ["a.BenchmarkPay", "a.ExamplePay", "a.FuzzPay"] {
        assert_eq!(
            reasons(&roots, &symbols, hook),
            [RootReason::Export, RootReason::FrameworkHook],
            "{hook}"
        );
    }
    assert_eq!(
        reasons(&roots, &symbols, "a.Benchmarking"),
        [RootReason::Export]
    );
}
