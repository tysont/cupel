; JavaScript and TypeScript symbols: functions, class methods, and functions bound to names.
; Shared by both languages, so node names must exist in the JavaScript, TypeScript, and TSX
; grammars. The earliest matching pattern wins for a definition.

; `#name` members are private to the class.
(class_body
  (method_definition
    name: (private_property_identifier) @symbol.name
    body: (statement_block) @symbol.body) @symbol.method @visibility.private)

; Every other method, including constructors and static methods.
(class_body
  (method_definition
    name: (_) @symbol.name
    body: (statement_block) @symbol.body) @symbol.method @visibility.public)

; Exported functions and exported functions bound to names.
(export_statement
  declaration: (function_declaration
    name: (identifier) @symbol.name
    body: (statement_block) @symbol.body) @symbol.function) @visibility.public

(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (identifier) @symbol.name
      value: [
        (arrow_function body: (_) @symbol.body)
        (function_expression body: (statement_block) @symbol.body)
      ]) @symbol.closure)) @visibility.public

; Everything else is private to its module.
(function_declaration
  name: (identifier) @symbol.name
  body: (statement_block) @symbol.body) @symbol.function

(variable_declarator
  name: (identifier) @symbol.name
  value: [
    (arrow_function body: (_) @symbol.body)
    (function_expression body: (statement_block) @symbol.body)
  ]) @symbol.closure

; Classes namespace their members.
(class_declaration
  name: (_) @scope.name) @scope
