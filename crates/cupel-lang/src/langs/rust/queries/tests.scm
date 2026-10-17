; Rust tests: functions marked `#[test]` (or `#[tokio::test]`), their assertion macros, skip and
; panic attributes, mockall doubles, and rstest fixtures. Attributes are siblings before the
; function they mark, so case patterns match the attribute, any attributes between, and the
; function; every capture in such a match belongs to that case.

((attribute_item
  (attribute (identifier) @_test (#eq? @_test "test"))) @framework.rust-test
  .
  (attribute_item)*
  .
  (function_item
    name: (identifier) @test.name) @test.case)

((attribute_item
  (attribute
    (scoped_identifier
      path: (identifier) @_runtime
      name: (identifier) @_test)
    (#eq? @_runtime "tokio")
    (#eq? @_test "test"))) @framework.tokio
  .
  (attribute_item)*
  .
  (function_item
    name: (identifier) @test.name) @test.case)

((attribute_item
  (attribute
    (identifier) @_ignore
    (#eq? @_ignore "ignore")
    value: (string_literal (string_content) @skip.reason)?)) @test.skip
  .
  (attribute_item)*
  .
  (function_item
    name: (identifier) @test.name) @test.case)

((attribute_item
  (attribute (identifier) @_panic (#eq? @_panic "should_panic"))) @test.assertion @assertion.idiom.should-panic
  .
  (attribute_item)*
  .
  (function_item
    name: (identifier) @test.name) @test.case)

; Property tests (ADR-095). Each `fn` in a `proptest! { … }` block is a test of its own; the block is
; a token tree, so the case is the function's body token tree, named by the identifier after `fn`.
(macro_invocation
  macro: (identifier) @_proptest
  (token_tree
    "fn"
    .
    (identifier) @test.name
    .
    (token_tree)
    .
    (token_tree) @test.case @property.proptest)
  (#eq? @_proptest "proptest"))

; `proptest!(|(x in …)| { … })` inside a test function makes it a property test.
(macro_invocation
  macro: (identifier) @_proptest
  (#eq? @_proptest "proptest")) @property.proptest

; Assertion macros in a `proptest!` body (or closure), at its top level and one block in: inside a
; token tree a macro call is its name, `!`, and its arguments.
(macro_invocation
  macro: (identifier) @_proptest
  (token_tree
    (token_tree
      (identifier) @test.assertion @assertion.name
      .
      "!"
      .
      (token_tree)))
  (#eq? @_proptest "proptest")
  (#any-of? @assertion.name "prop_assert" "prop_assert_eq" "prop_assert_ne" "assert" "assert_eq" "assert_ne"))

(macro_invocation
  macro: (identifier) @_proptest
  (token_tree
    (token_tree
      (token_tree
        (identifier) @test.assertion @assertion.name
        .
        "!"
        .
        (token_tree))))
  (#eq? @_proptest "proptest")
  (#any-of? @assertion.name "prop_assert" "prop_assert_eq" "prop_assert_ne" "assert" "assert_eq" "assert_ne"))

; `#[proptest]` (test-strategy) and `#[quickcheck]` (quickcheck_macros) make a function a test whose
; arguments the framework generates; a `#[quickcheck]` function's result is what quickcheck checks.
((attribute_item
  (attribute (identifier) @_proptest (#eq? @_proptest "proptest"))) @property.proptest
  .
  (attribute_item)*
  .
  (function_item
    name: (identifier) @test.name) @test.case)

((attribute_item
  (attribute (identifier) @_quickcheck (#eq? @_quickcheck "quickcheck"))) @property.quickcheck
  .
  (attribute_item)*
  .
  (function_item
    name: (identifier) @test.name) @test.case)

((attribute_item
  (attribute (identifier) @_quickcheck (#eq? @_quickcheck "quickcheck")))
  .
  (attribute_item)*
  .
  (function_item
    return_type: (_) @test.assertion @assertion.idiom.process) @test.case)

; `quickcheck(prop as fn(…) -> bool)` and `QuickCheck::new().quickcheck(…)` in a test check a
; property, failing the test with the shrunk arguments.
(call_expression
  function: [
    (identifier) @_quickcheck
    (scoped_identifier name: (identifier) @_quickcheck)
    (field_expression field: (field_identifier) @_quickcheck)
  ]
  (#eq? @_quickcheck "quickcheck")) @test.assertion @assertion.idiom.process @property.quickcheck

; Assertion idioms come before plain assertion macros: the earliest pattern wins. A `==` at the top
; level of `assert!`'s arguments compares two values, as `assert_eq!` does; a `!=` rules one out.
(macro_invocation
  macro: (identifier) @_assert
  (#eq? @_assert "assert")
  (token_tree "==")) @test.assertion @assertion.idiom.equal

(macro_invocation
  macro: (identifier) @_assert
  (#eq? @_assert "assert")
  (token_tree "!=")) @test.assertion @assertion.idiom.not-equal

; `assert!(x.is_none())` and `assert!(x.is_empty())` hold for exactly one value of `x`'s type, so
; they pin it; a leading `!` rules that one value out instead.
(macro_invocation
  macro: (identifier) @_assert
  (#eq? @_assert "assert")
  (token_tree
    (identifier) @_method
    (#any-of? @_method "is_none" "is_empty")) @_arguments
  (#not-match? @_arguments "^[(]\\s*!")) @test.assertion @assertion.idiom.one-value

(macro_invocation
  macro: (identifier) @_assert
  (#eq? @_assert "assert")
  (token_tree
    (identifier) @_method
    (#any-of? @_method "is_some" "is_ok"))) @test.assertion @assertion.idiom.is-present

(macro_invocation
  macro: (identifier) @_assert
  (#eq? @_assert "assert")
  (token_tree
    (identifier) @_method
    (#eq? @_method "is_err"))) @test.assertion @assertion.idiom.is-err

(macro_invocation
  macro: (identifier) @assertion.name
  (#match? @assertion.name "^(debug_|prop_)?assert")) @test.assertion

(macro_invocation
  macro: (scoped_identifier
    name: (identifier) @assertion.name)
  (#match? @assertion.name "^(debug_|prop_)?assert")) @test.assertion

; Assertion helpers: by convention a function named `assert_…` asserts, called plainly or with a
; turbofish. Its name is not in [tests.assertions], so it is `unknown_assertion` (custom, strong).
(call_expression
  function: [
    (identifier) @assertion.name
    (scoped_identifier name: (identifier) @assertion.name)
    (generic_function
      function: [
        (identifier) @assertion.name
        (scoped_identifier name: (identifier) @assertion.name)
      ])
  ]
  (#match? @assertion.name "^assert_")) @test.assertion

; assert_cmd: `Command::assert()` checks the process outcome its chained calls describe.
(call_expression
  function: (field_expression
    field: (field_identifier) @_assert)
  (#eq? @_assert "assert")) @test.assertion @assertion.idiom.process

; mockall: `MockThing::new()`.
(call_expression
  function: (scoped_identifier
    path: (identifier) @mock.target
    name: (identifier) @_new)
  (#match? @mock.target "^Mock[A-Z]")
  (#eq? @_new "new")) @test.mock

; rstest fixtures.
((attribute_item
  (attribute (identifier) @_fixture (#eq? @_fixture "fixture")))
  .
  (attribute_item)*
  .
  (function_item) @test.setup)
