; Python tests: pytest functions and `Test*` class methods, unittest `TestCase` methods, their
; `assert` statements and assert methods, skip decorators and calls, `patch`/`mocker`/`monkeypatch`
; and `Mock` doubles, and fixtures and setup methods (ADR-034). Decorators sit outside the function
; they mark, so decorator patterns also capture the function; every capture in such a match
; belongs to that case. A file with a `TestCase` subclass is unittest's; otherwise pytest's.

; unittest: `test*` methods of a `TestCase` subclass.
(class_definition
  superclasses: (argument_list
    [
      (identifier) @_base
      (attribute attribute: (identifier) @_base)
    ])
  body: (block
    [
      (function_definition
        name: (identifier) @test.name) @test.case
      (decorated_definition
        definition: (function_definition
          name: (identifier) @test.name) @test.case)
    ])
  (#match? @_base "TestCase$")
  (#match? @test.name "^test")) @framework.unittest

; pytest: `test*` methods of `Test*` classes, and `test*` functions at module level.
(class_definition
  name: (identifier) @_class
  body: (block
    [
      (function_definition
        name: (identifier) @test.name) @test.case
      (decorated_definition
        definition: (function_definition
          name: (identifier) @test.name) @test.case)
    ])
  (#match? @_class "^Test")
  (#match? @test.name "^test"))

(module
  [
    (function_definition
      name: (identifier) @test.name) @test.case
    (decorated_definition
      definition: (function_definition
        name: (identifier) @test.name) @test.case)
  ]
  (#match? @test.name "^test"))

; Property tests: Hypothesis's `@given(…)` (or `@hypothesis.given(…)`) generates the case's inputs
; (ADR-095).
(decorated_definition
  (decorator
    (call
      function: [
        (identifier) @_given
        (attribute attribute: (identifier) @_given)
      ])) @property.hypothesis
  definition: (function_definition
    name: (identifier) @test.name) @test.case
  (#eq? @_given "given")
  (#match? @test.name "^test"))

; Skip decorators: `@pytest.mark.skip(reason=…)`, `skipif`, `xfail`; `@unittest.skip("…")`,
; `skipIf`, `skipUnless`, `expectedFailure`.
(decorated_definition
  (decorator
    [
      (attribute) @_marker
      (call
        function: (attribute) @_marker
        arguments: (argument_list
          (keyword_argument
            name: (identifier) @_reason
            value: (string (string_content) @skip.reason))?))
    ]) @test.skip
  definition: (function_definition
    name: (identifier) @test.name) @test.case
  (#match? @_marker "^pytest\\.mark\\.(skip|skipif|xfail)$")
  (#eq? @_reason "reason")
  (#match? @test.name "^test"))

(decorated_definition
  (decorator
    [
      (identifier) @_marker
      (attribute) @_marker
      (call
        function: [(identifier) (attribute)] @_marker
        arguments: (argument_list (string (string_content) @skip.reason)? .))
    ]) @test.skip
  definition: (function_definition
    name: (identifier) @test.name) @test.case
  (#match? @_marker "^(unittest\\.)?(skip|skipIf|skipUnless|expectedFailure)$")
  (#match? @test.name "^test"))

; `@patch("module.name")`, `@mock.patch.object(Class, "name")`, … on a case.
(decorated_definition
  (decorator
    (call
      function: [(identifier) (attribute)] @_patch
      arguments: (argument_list
        .
        [
          (string (string_content) @mock.target)
          (identifier) @mock.target
          (attribute) @mock.target
        ]))) @test.mock
  definition: (function_definition
    name: (identifier) @test.name) @test.case
  (#match? @_patch "^((unittest\\.)?mock\\.)?patch(\\.object|\\.dict|\\.multiple)?$")
  (#match? @test.name "^test"))

; Assert statements. Idioms come before the bare `assert x`: the earliest pattern wins.
(assert_statement
  .
  (comparison_operator
    (call
      function: [
        (identifier) @_approx
        (attribute attribute: (identifier) @_approx)
      ]))
  (#eq? @_approx "approx")) @test.assertion @assertion.idiom.approx

(assert_statement
  .
  (comparison_operator
    (identifier) @_snapshot)
  (#eq? @_snapshot "snapshot")) @test.assertion @assertion.idiom.snapshot

(assert_statement
  .
  (comparison_operator
    operators: "is not"
    (none))) @test.assertion @assertion.idiom.is-not-none

(assert_statement
  .
  (comparison_operator
    operators: "is"
    (none))) @test.assertion @assertion.idiom.is-none

(assert_statement
  .
  (comparison_operator
    operators: ["==" "is"])) @test.assertion @assertion.idiom.equal

(assert_statement
  .
  (comparison_operator
    operators: ["!=" "is not"])) @test.assertion @assertion.idiom.not-equal

(assert_statement
  .
  (comparison_operator
    operators: ["<" "<=" ">" ">="])) @test.assertion @assertion.idiom.ordering

(assert_statement
  .
  (comparison_operator
    operators: ["in" "not in"])) @test.assertion @assertion.idiom.membership

(assert_statement) @test.assertion @assertion.idiom.truthy

; `pytest.raises(…)`, as a context manager or a call.
(call
  function: (attribute
    object: (identifier) @_pytest
    attribute: (identifier) @_raises)
  (#eq? @_pytest "pytest")
  (#eq? @_raises "raises")) @test.assertion @assertion.idiom.raises

; unittest assert methods, keyed by name.
(call
  function: (attribute
    object: (identifier) @_self
    attribute: (identifier) @assertion.name)
  (#eq? @_self "self")
  (#match? @assertion.name "^assert")) @test.assertion

; Mock assertions (`m.assert_called_once_with(…)`) and `snapshot.assert_match(…)`, keyed by name.
(call
  function: (attribute
    attribute: (identifier) @assertion.name)
  (#match? @assertion.name "^assert_")) @test.assertion

; Skips inside a case: `pytest.skip("…")`, `self.skipTest("…")`.
(call
  function: (attribute) @_skip
  arguments: (argument_list
    .
    (string (string_content) @skip.reason)?)
  (#match? @_skip "^(pytest\\.skip|self\\.skipTest)$")) @test.skip

; Test doubles inside a case: `with patch(…)`, `mocker.patch(…)`, `monkeypatch.setattr(…)`, and
; `Mock()`/`MagicMock()` constructions.
(with_item
  value: (call
    function: [(identifier) (attribute)] @_patch
    arguments: (argument_list
      .
      [
        (string (string_content) @mock.target)
        (identifier) @mock.target
        (attribute) @mock.target
      ])) @test.mock
  (#match? @_patch "^((unittest\\.)?mock\\.)?patch(\\.object|\\.dict|\\.multiple)?$"))

(call
  function: (attribute
    object: (identifier) @_fixture) @_patch
  arguments: (argument_list
    .
    [
      (string (string_content) @mock.target)
      (identifier) @mock.target
      (attribute) @mock.target
    ])
  (#any-of? @_fixture "mocker" "monkeypatch")
  (#match? @_patch "^(mocker\\.patch(\\.object|\\.dict|\\.multiple)?|monkeypatch\\.(setattr|setitem|setenv|delattr|delitem|delenv))$")) @test.mock

(call
  function: [
    (identifier) @mock.target
    (attribute attribute: (identifier) @mock.target)
  ]
  (#any-of? @mock.target "Mock" "MagicMock" "AsyncMock" "NonCallableMock")) @test.mock

; Fixtures and setup methods.
(decorated_definition
  (decorator
    [
      (identifier)
      (attribute)
      (call function: [(identifier) (attribute)])
    ] @_fixture)
  definition: (function_definition) @test.setup
  (#match? @_fixture "^(pytest\\.)?fixture\\b"))

(function_definition
  name: (identifier) @_setup
  (#any-of? @_setup
    "setUp" "asyncSetUp" "setUpClass" "setUpModule"
    "setup_method" "setup_class" "setup_function" "setup_module")) @test.setup

; Docstrings (ADR-134): a test function's first statement, when it is a string. It documents
; only a case the patterns above found.
(function_definition
  body: (block
    .
    (expression_statement
      .
      (string) @test.doc
      .))) @doc.target
