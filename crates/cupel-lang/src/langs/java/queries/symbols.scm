; Java symbols: methods and constructors of classes, interfaces, enums, and records, and lambdas
; bound to names. Every named function in Java is a method; `static` ones too. The package clause
; names the namespace, and each type declaration scopes what it declares. The earliest matching
; pattern wins for a definition, so modifiers are matched before the defaults.

(package_declaration
  [(identifier) (scoped_identifier)] @namespace.name)

[
  (class_declaration name: (identifier) @scope.name)
  (interface_declaration name: (identifier) @scope.name)
  (enum_declaration name: (identifier) @scope.name)
  (record_declaration name: (identifier) @scope.name)
  (annotation_type_declaration name: (identifier) @scope.name)
] @scope

; `private` members are private, `public` and `protected` ones are visible outside the package
; (to subclasses, for `protected`), and the rest are package-private: `internal` (ADR-108).
(method_declaration
  (modifiers "private")
  name: (identifier) @symbol.name
  body: (block) @symbol.body) @symbol.method @visibility.private

(method_declaration
  (modifiers ["public" "protected"])
  name: (identifier) @symbol.name
  body: (block) @symbol.body) @symbol.method @visibility.public

; An interface's methods are public unless they say `private`.
(interface_body
  (method_declaration
    name: (identifier) @symbol.name
    body: (block) @symbol.body) @symbol.method @visibility.public)

(method_declaration
  name: (identifier) @symbol.name
  body: (block) @symbol.body) @symbol.method

(constructor_declaration
  (modifiers "private")
  name: (identifier) @symbol.name
  body: (constructor_body) @symbol.body) @symbol.method @visibility.private

(constructor_declaration
  (modifiers ["public" "protected"])
  name: (identifier) @symbol.name
  body: (constructor_body) @symbol.body) @symbol.method @visibility.public

(constructor_declaration
  name: (identifier) @symbol.name
  body: (constructor_body) @symbol.body) @symbol.method

; A record's compact constructor (`Name { … }`) is its canonical constructor.
(compact_constructor_declaration
  name: (identifier) @symbol.name
  body: (block) @symbol.body) @symbol.method @visibility.public

; Lambdas bound to names: a local variable or a field whose value is a lambda.
(local_variable_declaration
  declarator: (variable_declarator
    name: (identifier) @symbol.name
    value: (lambda_expression body: (_) @symbol.body))) @symbol.closure @visibility.private

(field_declaration
  (modifiers "private")
  declarator: (variable_declarator
    name: (identifier) @symbol.name
    value: (lambda_expression body: (_) @symbol.body))) @symbol.closure @visibility.private

(field_declaration
  (modifiers ["public" "protected"])
  declarator: (variable_declarator
    name: (identifier) @symbol.name
    value: (lambda_expression body: (_) @symbol.body))) @symbol.closure @visibility.public

(field_declaration
  declarator: (variable_declarator
    name: (identifier) @symbol.name
    value: (lambda_expression body: (_) @symbol.body))) @symbol.closure

; Parameters: each declared parameter, varargs included; a receiver parameter (`Type this`) is
; not one, and neither is the implicit `this`.
(formal_parameters
  [
    (formal_parameter)
    (spread_parameter)
  ] @symbol.parameter)

(lambda_expression
  parameters: (identifier) @symbol.parameter)

(lambda_expression
  parameters: (inferred_parameters
    (identifier) @symbol.parameter))
