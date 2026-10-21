; Java tests for JUnit 5 (Jupiter), JUnit 4, and jqwik: methods annotated `@Test` (and its
; parameterized, repeated, and factory forms) or `@Property`, with JUnit and AssertJ assertions,
; Mockito doubles, `@Disabled`/`@Ignore` skips, and lifecycle methods as setup. Earliest
; patterns win: frameworks by import, then idioms before assertion names.

; The framework: jqwik, JUnit 5, or JUnit 4, by what the file imports.
(import_declaration
  (scoped_identifier) @_path
  (#match? @_path "^net\\.jqwik\\.")) @framework.jqwik

(import_declaration
  (scoped_identifier) @_path
  (#match? @_path "^org\\.junit\\.jupiter\\.")) @framework.junit5

(import_declaration
  (scoped_identifier) @_path
  (#match? @_path "^org\\.junit\\.")) @framework.junit4

; Cases: a method whose annotations mark it a test.
(method_declaration
  (modifiers
    [
      (marker_annotation name: (identifier) @_test)
      (annotation name: (identifier) @_test)
    ])
  name: (identifier) @test.name
  (#any-of? @_test "Test" "ParameterizedTest" "RepeatedTest" "TestFactory" "TestTemplate" "Example")) @test.case

; Property tests: jqwik `@Property` methods, whose inputs jqwik generates (ADR-095). A property
; that returns a boolean asserts it.
(method_declaration
  (modifiers
    [
      (marker_annotation name: (identifier) @_property)
      (annotation name: (identifier) @_property)
    ])
  name: (identifier) @test.name
  (#eq? @_property "Property")) @test.case @property.jqwik

(method_declaration
  (modifiers
    [
      (marker_annotation name: (identifier) @_property)
      (annotation name: (identifier) @_property)
    ])
  type: (boolean_type)
  body: (block
    (return_statement) @test.assertion @assertion.idiom.property)
  (#eq? @_property "Property"))

; JUnit 4's `@Test(expected = X.class)`: the case must throw.
(method_declaration
  (modifiers
    (annotation
      name: (identifier) @_test
      arguments: (annotation_argument_list
        (element_value_pair
          key: (identifier) @_expected)) @test.assertion @assertion.idiom.expected))
  (#eq? @_test "Test")
  (#eq? @_expected "expected"))

; Skips: `@Disabled` (JUnit 5) and `@Ignore` (JUnit 4), with or without a reason.
(method_declaration
  (modifiers
    (annotation
      name: (identifier) @_skip
      arguments: (annotation_argument_list
        .
        (string_literal) @skip.reason)) @test.skip)
  (#any-of? @_skip "Disabled" "Ignore"))

(method_declaration
  (modifiers
    (marker_annotation
      name: (identifier) @_skip) @test.skip)
  (#any-of? @_skip "Disabled" "Ignore"))

; Setup: lifecycle methods shared by a file's cases.
(method_declaration
  (modifiers
    (marker_annotation
      name: (identifier) @_setup))
  (#any-of? @_setup "BeforeEach" "BeforeAll" "AfterEach" "AfterAll" "Before" "BeforeClass" "After" "AfterClass" "BeforeProperty" "BeforeContainer")) @test.setup

; AssertJ: `assertThatThrownBy(…)` and `assertThatExceptionOfType(…)` start a throws assertion;
; `assertThat(x).isEqualTo(y)` is classified by the first matcher of the chain.
(method_invocation
  object: (method_invocation
    name: (identifier) @_start)
  (#any-of? @_start "assertThatThrownBy" "assertThatExceptionOfType" "assertThatCode")) @test.assertion @assertion.idiom.thrown

(method_invocation
  object: (method_invocation
    name: (identifier) @_start)
  name: (identifier) @assertion.name
  (#match? @_start "^assertThat")) @test.assertion

; Snapshots (java-snapshot-testing): `expect.toMatchSnapshot(…)`.
(method_invocation
  name: (identifier) @assertion.name
  (#eq? @assertion.name "toMatchSnapshot")) @test.assertion

; JUnit's static assertions, called bare or through `Assertions`/`Assert`; `fail()` too.
(method_invocation
  name: (identifier) @assertion.name
  (#match? @assertion.name "^(assert|fail$)")
  (#not-match? @assertion.name "^assertThat")) @test.assertion

; Mockito verification asserts that a double was called.
(method_invocation
  name: (identifier) @assertion.name
  (#any-of? @assertion.name "verify" "verifyNoInteractions" "verifyNoMoreInteractions")) @test.assertion

; Doubles: Mockito's `mock(X.class)` and `spy(…)`, and fields annotated `@Mock` or `@Spy`.
(method_invocation
  name: (identifier) @_mock
  arguments: (argument_list
    .
    (class_literal
      [
        (type_identifier) @mock.target
        (scoped_type_identifier) @mock.target
      ]))
  (#any-of? @_mock "mock" "spy")) @test.mock

(field_declaration
  (modifiers
    (marker_annotation
      name: (identifier) @_mock))
  type: (_) @mock.target
  (#any-of? @_mock "Mock" "Spy" "MockBean")) @test.mock

; Doc comments (ADR-134): a `/** … */` Javadoc directly above a test method (its annotations are
; part of the method). It documents only a case the patterns above found.
((block_comment) @test.doc
  .
  (method_declaration) @doc.target
  (#match? @test.doc "^/\\*\\*"))
