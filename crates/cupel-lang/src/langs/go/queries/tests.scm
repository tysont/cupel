; Go tests: `func TestX(t *testing.T)` functions, the failure calls (`t.Error*`, `t.Fatal*`,
; `t.Fail*`) classified by the `if` condition that guards them, testify and go-snaps assertions,
; testify mock expectations, `t.Skip*`, gomock and testify doubles, and `TestMain` and
; `t.Cleanup` setup. Subtests run by `t.Run` belong to their test function: their names are
; usually computed, and the function is what the runner and coverage report name.

; The framework: testify when a file imports it, else the standard library's runner.
(import_spec
  path: (interpreted_string_literal) @_path
  (#match? @_path "^\"github.com/stretchr/testify/")) @framework.testify

(function_declaration
  name: (identifier) @test.name
  parameters: (parameter_list
    .
    (parameter_declaration
      type: (pointer_type
        (qualified_type
          package: (package_identifier) @_testing
          name: (type_identifier) @_t)))
    .)
  (#match? @test.name "^Test([^a-z]|$)")
  (#eq? @_testing "testing")
  (#eq? @_t "T")) @test.case @framework.go-test

; Failure calls guarded by an `if`, most specific condition first: the earliest pattern wins
; for a call. The call is the assertion; the condition says what it asserts.

; `if err == nil { t.Error(…) }`: the code must fail.
(if_statement
  condition: (binary_expression
    left: (identifier) @_err
    operator: "=="
    right: (nil))
  consequence: (block
    (statement_list
      (expression_statement
        (call_expression
          function: (selector_expression
            operand: (identifier) @_t
            field: (field_identifier) @_fail)) @test.assertion @assertion.idiom.error)))
  (#match? @_err "^err")
  (#any-of? @_t "t" "b" "tb")
  (#any-of? @_fail "Error" "Errorf" "Fatal" "Fatalf" "Fail" "FailNow"))

; `if err != nil { t.Fatal(err) }`: the code must succeed.
(if_statement
  condition: (binary_expression
    left: (identifier) @_err
    operator: "!="
    right: (nil))
  consequence: (block
    (statement_list
      (expression_statement
        (call_expression
          function: (selector_expression
            operand: (identifier) @_t
            field: (field_identifier) @_fail)) @test.assertion @assertion.idiom.no-error)))
  (#match? @_err "^err")
  (#any-of? @_t "t" "b" "tb")
  (#any-of? @_fail "Error" "Errorf" "Fatal" "Fatalf" "Fail" "FailNow"))

; `if x == nil { t.Fatal(…) }`: a value must be present.
(if_statement
  condition: (binary_expression
    operator: "=="
    right: (nil))
  consequence: (block
    (statement_list
      (expression_statement
        (call_expression
          function: (selector_expression
            operand: (identifier) @_t
            field: (field_identifier) @_fail)) @test.assertion @assertion.idiom.not-nil)))
  (#any-of? @_t "t" "b" "tb")
  (#any-of? @_fail "Error" "Errorf" "Fatal" "Fatalf" "Fail" "FailNow"))

; `if got != want { t.Errorf(…) }`: one value is pinned.
(if_statement
  condition: (binary_expression
    operator: "!="
    right: (_) @_want)
  consequence: (block
    (statement_list
      (expression_statement
        (call_expression
          function: (selector_expression
            operand: (identifier) @_t
            field: (field_identifier) @_fail)) @test.assertion @assertion.idiom.equal)))
  (#not-eq? @_want "nil")
  (#any-of? @_t "t" "b" "tb")
  (#any-of? @_fail "Error" "Errorf" "Fatal" "Fatalf" "Fail" "FailNow"))

; `if got == unwanted { t.Errorf(…) }`: one value is ruled out.
(if_statement
  condition: (binary_expression
    operator: "=="
    right: (_) @_unwanted)
  consequence: (block
    (statement_list
      (expression_statement
        (call_expression
          function: (selector_expression
            operand: (identifier) @_t
            field: (field_identifier) @_fail)) @test.assertion @assertion.idiom.not-equal)))
  (#not-eq? @_unwanted "nil")
  (#any-of? @_t "t" "b" "tb")
  (#any-of? @_fail "Error" "Errorf" "Fatal" "Fatalf" "Fail" "FailNow"))

; Any other failure call: some condition held.
(call_expression
  function: (selector_expression
    operand: (identifier) @_t
    field: (field_identifier) @_fail)
  (#any-of? @_t "t" "b" "tb")
  (#any-of? @_fail "Error" "Errorf" "Fatal" "Fatalf" "Fail" "FailNow")) @test.assertion @assertion.idiom.fails

; Property tests: `quick.Check(…)` and `quick.CheckEqual(…)` generate the case's inputs with
; testing/quick (ADR-095); the check is also an assertion, whose error the test reports.
(call_expression
  function: (selector_expression
    operand: (identifier) @_quick
    field: (field_identifier) @_check)
  (#eq? @_quick "quick")
  (#any-of? @_check "Check" "CheckEqual")) @property.testing-quick @test.assertion @assertion.idiom.property

; testify (`assert.Equal`, `require.NoError`) and go-snaps (`snaps.MatchSnapshot`).
(call_expression
  function: (selector_expression
    operand: (identifier) @_package
    field: (field_identifier) @assertion.name)
  (#any-of? @_package "assert" "require" "snaps")) @test.assertion

; testify mock expectations.
(call_expression
  function: (selector_expression
    field: (field_identifier) @assertion.name)
  (#any-of? @assertion.name "AssertExpectations" "AssertCalled" "AssertNotCalled" "AssertNumberOfCalls")) @test.assertion

; Doubles: gomock and mockery constructors, and testify mocks built as `&MockX{}`.
(call_expression
  function: [
    (identifier) @mock.target
    (selector_expression field: (field_identifier) @mock.target)
  ]
  (#match? @mock.target "^NewMock[A-Z]")) @test.mock

(composite_literal
  type: (type_identifier) @mock.target
  (#match? @mock.target "^Mock[A-Z]")) @test.mock

(call_expression
  function: (selector_expression
    operand: (identifier) @_t
    field: (field_identifier) @_skip)
  arguments: (argument_list
    .
    [
      (interpreted_string_literal)
      (raw_string_literal)
    ]? @skip.reason)
  (#any-of? @_t "t" "b" "tb")
  (#any-of? @_skip "Skip" "Skipf" "SkipNow")) @test.skip

(function_declaration
  name: (identifier) @_main
  (#eq? @_main "TestMain")) @test.setup

(call_expression
  function: (selector_expression
    operand: (identifier) @_t
    field: (field_identifier) @_cleanup)
  (#any-of? @_t "t" "b" "tb")
  (#eq? @_cleanup "Cleanup")) @test.setup

; Doc comments (ADR-134): the `//` lines directly above a test function. It documents only a
; case the patterns above found.
((comment)+ @test.doc
  .
  (function_declaration) @doc.target)
