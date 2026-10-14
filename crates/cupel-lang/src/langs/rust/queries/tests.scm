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
