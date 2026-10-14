; TypeScript-only symbol patterns. They come before the shared ECMAScript patterns, so they win
; for the definitions they match.

; Methods with an accessibility modifier take their visibility from it.
(class_body
  (method_definition
    (accessibility_modifier) @symbol.visibility
    name: (_) @symbol.name
    body: (statement_block) @symbol.body) @symbol.method)

; `export = name` exports the named definition.
(export_statement
  .
  "="
  .
  (identifier) @export.name)

; Abstract classes and namespaces (`namespace A {}`, `module A {}`) namespace their members.
(abstract_class_declaration
  name: (_) @scope.name) @scope

(internal_module
  name: (_) @scope.name) @scope

(module
  name: (_) @scope.name) @scope

; Parameters: each declared parameter; a `this` parameter types the receiver and is not one.
(formal_parameters
  (required_parameter) @symbol.parameter
  (#not-match? @symbol.parameter "^this\\b"))

(formal_parameters
  (optional_parameter) @symbol.parameter)
