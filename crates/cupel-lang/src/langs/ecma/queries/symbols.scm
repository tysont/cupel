; JavaScript and TypeScript symbols: functions, generators, class methods, functions bound to
; names, CommonJS export assignments, default exports, and test callbacks. Shared by both
; languages, so node names must exist in the JavaScript, TypeScript, and TSX grammars. The
; earliest matching pattern wins for a definition.

; `#name` members are private to the class.
(class_body
  (method_definition
    name: (private_property_identifier) @symbol.name
    body: (statement_block) @symbol.body) @symbol.method @visibility.private)

; Every other method, including constructors, accessors, and static methods.
(class_body
  (method_definition
    name: (_) @symbol.name
    body: (statement_block) @symbol.body) @symbol.method @visibility.public)

; Exported functions and exported functions bound to names.
(export_statement
  declaration: [
    (function_declaration
      name: (identifier) @symbol.name
      body: (statement_block) @symbol.body)
    (generator_function_declaration
      name: (identifier) @symbol.name
      body: (statement_block) @symbol.body)
  ] @symbol.function) @visibility.public

(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (identifier) @symbol.name
      value: [
        (arrow_function body: (_) @symbol.body)
        (function_expression body: (statement_block) @symbol.body)
      ]) @symbol.closure)) @visibility.public

; An anonymous default export is called `default`, the name importers see.
(export_statement
  "default" @symbol.name
  value: [
    (arrow_function body: (_) @symbol.body)
    (function_expression body: (statement_block) @symbol.body)
  ] @symbol.function) @visibility.public

; CommonJS: `exports.name = function …` and `module.exports.name = …`.
(assignment_expression
  left: (member_expression
    object: [
      (identifier) @_exports
      (member_expression
        object: (identifier) @_module
        property: (property_identifier) @_exports)
    ]
    property: (property_identifier) @symbol.name)
  right: [
    (arrow_function body: (_) @symbol.body)
    (function_expression body: (statement_block) @symbol.body)
  ]
  (#eq? @_exports "exports")
  (#eq? @_module "module")) @symbol.closure @visibility.public

; Names exported apart from their definition: `export { a, b as c }` (not re-exports from
; another module), `export default a`, `module.exports = { a, b: c }`, `module.exports = a`,
; and `exports.x = a`.
(export_statement
  !source
  (export_clause
    (export_specifier
      name: (identifier) @export.name)))

(export_statement
  value: (identifier) @export.name)

(assignment_expression
  left: (member_expression
    object: (identifier) @_module
    property: (property_identifier) @_exports)
  right: [
    (identifier) @export.name
    (object
      [
        (shorthand_property_identifier) @export.name
        (pair value: (identifier) @export.name)
      ])
  ]
  (#eq? @_module "module")
  (#eq? @_exports "exports"))

(assignment_expression
  left: (member_expression
    object: (identifier) @_exports
    property: (property_identifier))
  right: (identifier) @export.name
  (#eq? @_exports "exports"))

; Test callbacks are closures named by the test's title (`it("adds", () => …)`, `test.skip`,
; `xit`); suites (`describe`) namespace them. See ADR-030.
(call_expression
  function: [
    (identifier) @_test
    (member_expression
      object: (identifier) @_test
      property: (property_identifier))
  ]
  arguments: (arguments
    .
    (string (string_fragment) @symbol.name)
    .
    [
      (arrow_function body: (_) @symbol.body)
      (function_expression body: (statement_block) @symbol.body)
    ] @symbol.closure)
  (#any-of? @_test "it" "test" "specify" "xit" "xtest" "fit"))

(call_expression
  function: [
    (identifier) @_suite
    (member_expression
      object: (identifier) @_suite
      property: (property_identifier))
  ]
  arguments: (arguments
    .
    (string (string_fragment) @scope.name))
  (#any-of? @_suite "describe" "suite" "context" "xdescribe" "fdescribe")) @scope

; Everything else is private to its module.
[
  (function_declaration
    name: (identifier) @symbol.name
    body: (statement_block) @symbol.body)
  (generator_function_declaration
    name: (identifier) @symbol.name
    body: (statement_block) @symbol.body)
] @symbol.function

(variable_declarator
  name: (identifier) @symbol.name
  value: [
    (arrow_function body: (_) @symbol.body)
    (function_expression body: (statement_block) @symbol.body)
  ]) @symbol.closure

; Classes namespace their members.
(class_declaration
  name: (_) @scope.name) @scope
