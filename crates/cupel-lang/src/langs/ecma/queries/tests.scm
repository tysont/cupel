; JavaScript and TypeScript tests for vitest, jest, mocha (with chai and sinon), and node:test.
; A case is the callback passed to `it`/`test` after its title (the same node symbols.scm makes
; a closure, ADR-030). Earliest patterns win: frameworks by import first, then by usage;
; assertion idioms before plain matchers.

; Frameworks.
(import_statement
  source: (string (string_fragment) @_source)
  (#eq? @_source "vitest")) @framework.vitest

(member_expression
  object: (meta_property)
  property: (property_identifier) @_vitest
  (#eq? @_vitest "vitest")) @framework.vitest

(import_statement
  source: (string (string_fragment) @_source)
  (#eq? @_source "@jest/globals")) @framework.jest

(import_statement
  source: (string (string_fragment) @_source)
  (#eq? @_source "node:test")) @framework.node-test

(call_expression
  function: (identifier) @_require
  arguments: (arguments (string (string_fragment) @_source))
  (#eq? @_require "require")
  (#eq? @_source "node:test")) @framework.node-test

(import_statement
  source: (string (string_fragment) @_source)
  (#any-of? @_source "mocha" "chai")) @framework.mocha

(call_expression
  function: (identifier) @_require
  arguments: (arguments (string (string_fragment) @_source))
  (#eq? @_require "require")
  (#any-of? @_source "mocha" "chai")) @framework.mocha

(member_expression
  object: (identifier) @_vi
  (#eq? @_vi "vi")) @framework.vitest

(member_expression
  object: (identifier) @_jest
  (#eq? @_jest "jest")) @framework.jest

; Cases. `it.skip`, `test.skip`, and `it.todo` skip their case; `xit` and `xtest` too.
(call_expression
  function: (member_expression
    object: (identifier) @_test
    property: (property_identifier) @_skip)
  arguments: (arguments
    .
    (string) @test.name
    .
    [(arrow_function) (function_expression)] @test.case)
  (#any-of? @_test "it" "test" "specify")
  (#any-of? @_skip "skip" "todo")) @test.skip

(call_expression
  function: (identifier) @test.skip
  arguments: (arguments
    .
    (string) @test.name
    .
    [(arrow_function) (function_expression)] @test.case)
  (#any-of? @test.skip "xit" "xtest"))

(call_expression
  function: [
    (identifier) @_test
    (member_expression
      object: (identifier) @_test
      property: (property_identifier))
  ]
  arguments: (arguments
    .
    (string) @test.name
    .
    [(arrow_function) (function_expression)] @test.case)
  (#any-of? @_test "it" "test" "specify" "fit"))

; Property tests: fast-check's runners, `fc.assert(…)` and `fc.check(…)`, generate the case's
; inputs (ADR-095). `fc.assert` throws the shrunk counterexample, so it is also an assertion;
; `fc.check` returns it.
(call_expression
  function: (member_expression
    object: (identifier) @_fc
    property: (property_identifier) @_run)
  (#eq? @_fc "fc")
  (#eq? @_run "assert")) @property.fast-check @test.assertion @assertion.idiom.property

(call_expression
  function: (member_expression
    object: (identifier) @_fc
    property: (property_identifier) @_run)
  (#eq? @_fc "fc")
  (#eq? @_run "check")) @property.fast-check

; A skip at run time: `t.skip("why")` (node:test), `this.skip()` (mocha), `ctx.skip()`.
(call_expression
  function: (member_expression
    object: [(identifier) (this)] @_context
    property: (property_identifier) @_skip)
  arguments: (arguments
    .
    (string (string_fragment) @skip.reason)?)
  (#eq? @_skip "skip")
  (#not-any-of? @_context "it" "test" "specify" "describe" "suite")) @test.skip

; Assertion idioms on expect chains: negations are weak, `toThrow(expected)` is strong.
(call_expression
  function: (member_expression
    object: (member_expression
      object: (call_expression function: (identifier) @_expect)
      property: (property_identifier) @_not)
    property: (property_identifier) @_matcher)
  (#eq? @_expect "expect")
  (#eq? @_not "not")
  (#any-of? @_matcher "toBe" "toEqual" "toStrictEqual")) @test.assertion @assertion.idiom.not-equal

(call_expression
  function: (member_expression
    object: (member_expression
      object: (call_expression function: (identifier) @_expect)
      property: (property_identifier) @_not)
    property: (property_identifier) @_matcher)
  (#eq? @_expect "expect")
  (#eq? @_not "not")
  (#any-of? @_matcher "toBeNull" "toBeUndefined")) @test.assertion @assertion.idiom.not-nullish

(call_expression
  function: (member_expression
    object: (member_expression
      object: (call_expression function: (identifier) @_expect)
      property: (property_identifier) @_not)
    property: (property_identifier) @_matcher)
  (#eq? @_expect "expect")
  (#eq? @_not "not")
  (#any-of? @_matcher "toThrow" "toThrowError")) @test.assertion @assertion.idiom.does-not-throw

(call_expression
  function: (member_expression
    object: (member_expression
      object: (call_expression function: (identifier) @_expect)
      property: (property_identifier) @_not))
  (#eq? @_expect "expect")
  (#eq? @_not "not")) @test.assertion @assertion.idiom.negated

(call_expression
  function: (member_expression
    object: [
      (call_expression function: (identifier) @_expect)
      (member_expression
        object: (call_expression function: (identifier) @_expect))
    ]
    property: (property_identifier) @_matcher)
  arguments: (arguments (_))
  (#eq? @_expect "expect")
  (#any-of? @_matcher "toThrow" "toThrowError")) @test.assertion @assertion.idiom.throws-specific

(call_expression
  function: (member_expression
    object: (identifier) @_assert
    property: (property_identifier) @_method)
  arguments: (arguments (_) . (_))
  (#eq? @_assert "assert")
  (#any-of? @_method "throws" "rejects")) @test.assertion @assertion.idiom.throws-specific

; Expect chains up to three links deep: `expect(x).toBe(y)`, `expect(p).resolves.toBe(y)`,
; chai's `expect(x).to.equal(y)` and `expect(x).to.deep.equal(y)`. The key is the last link.
(call_expression
  function: (member_expression
    object: [
      (call_expression function: (identifier) @_expect)
      (member_expression
        object: (call_expression function: (identifier) @_expect))
      (member_expression
        object: (member_expression
          object: (call_expression function: (identifier) @_expect)))
    ]
    property: (property_identifier) @assertion.name)
  (#eq? @_expect "expect")) @test.assertion

; chai's property assertions: `expect(x).to.be.true`, `expect(x).to.exist`.
(expression_statement
  (member_expression
    object: [
      (member_expression
        object: (call_expression function: (identifier) @_expect))
      (member_expression
        object: (member_expression
          object: (call_expression function: (identifier) @_expect)))
    ]
    property: (property_identifier) @assertion.name) @test.assertion
  (#eq? @_expect "expect"))

; node:assert and chai's assert: `assert.equal(a, b)`, `assert(x)`.
(call_expression
  function: (member_expression
    object: (identifier) @_assert
    property: (property_identifier)) @assertion.name
  (#eq? @_assert "assert")) @test.assertion

(call_expression
  function: (identifier) @assertion.name
  (#eq? @assertion.name "assert")) @test.assertion

; Mocks: module mocks name the module, spies the method, other doubles their factory.
(call_expression
  function: (member_expression
    object: (identifier) @_library
    property: (property_identifier) @_mock)
  arguments: (arguments
    .
    (string (string_fragment) @mock.target))
  (#any-of? @_library "vi" "jest")
  (#any-of? @_mock "mock" "doMock")) @test.mock

(call_expression
  function: (member_expression
    object: (identifier) @_library
    property: (property_identifier) @_spy)
  arguments: (arguments
    .
    (_)
    .
    (string (string_fragment) @mock.target))
  (#any-of? @_library "vi" "jest")
  (#eq? @_spy "spyOn")) @test.mock

(call_expression
  function: (member_expression
    object: (identifier) @_library
    property: (property_identifier) @_double) @mock.target
  (#any-of? @_library "vi" "jest")
  (#eq? @_double "fn")) @test.mock

(call_expression
  function: (member_expression
    object: (identifier) @_library
    property: (property_identifier) @_double) @mock.target
  (#eq? @_library "sinon")
  (#any-of? @_double "stub" "spy" "mock" "fake")) @test.mock

(call_expression
  function: (member_expression
    object: [
      (identifier) @_mock
      (member_expression property: (property_identifier) @_mock)
    ]
    property: (property_identifier) @_double) @mock.target
  (#eq? @_mock "mock")
  (#any-of? @_double "fn" "method" "getter" "setter")) @test.mock

; Setup and teardown hooks shared by a file's cases.
(call_expression
  function: (identifier) @_hook
  (#any-of? @_hook "beforeEach" "beforeAll" "afterEach" "afterAll" "before" "after")) @test.setup
