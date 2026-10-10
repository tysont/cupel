; TypeScript-only symbol patterns. They come before the shared ECMAScript patterns, so they win
; for the definitions they match.

; Methods with an accessibility modifier take their visibility from it.
(class_body
  (method_definition
    (accessibility_modifier) @symbol.visibility
    name: (_) @symbol.name
    body: (statement_block) @symbol.body) @symbol.method)

; Abstract classes and namespaces (`namespace A {}`, `module A {}`) namespace their members.
(abstract_class_declaration
  name: (_) @scope.name) @scope

(internal_module
  name: (_) @scope.name) @scope

(module
  name: (_) @scope.name) @scope
