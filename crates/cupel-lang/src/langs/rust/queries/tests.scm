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

; Assertion idioms come before plain assertion macros: the earliest pattern wins.
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
