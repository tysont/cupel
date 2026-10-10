; Python symbols: functions, methods (decorated or not), nested functions, and lambdas bound to
; names. The earliest matching pattern wins for a definition, so the `__all__` patterns come first:
; a module-level name listed in `__all__` is public; with `__all__` present, an unlisted name is
; internal unless an underscore rule applies; then nested definitions are private; then methods
; and everything else take their visibility from their name (ADR-034).

; Listed in a module-level `__all__` (assigned before the definitions, as PEP 8 places it).
(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [
        (list (string (string_content) @_exported))
        (tuple (string (string_content) @_exported))
      ]))
  [
    (function_definition
      name: (identifier) @symbol.name
      body: (block) @symbol.body) @symbol.function
    (decorated_definition
      definition: (function_definition
        name: (identifier) @symbol.name
        body: (block) @symbol.body) @symbol.function)
    (expression_statement
      (assignment
        left: (identifier) @symbol.name
        right: (lambda body: (_) @symbol.body)) @symbol.closure)
  ] @visibility.public
  (#eq? @_all "__all__")
  (#eq? @_exported @symbol.name))

; Not listed in a module-level `__all__`, and named without underscores.
(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [(list) (tuple)]))
  [
    (function_definition
      name: (identifier) @symbol.name
      body: (block) @symbol.body) @symbol.function
    (decorated_definition
      definition: (function_definition
        name: (identifier) @symbol.name
        body: (block) @symbol.body) @symbol.function)
    (expression_statement
      (assignment
        left: (identifier) @symbol.name
        right: (lambda body: (_) @symbol.body)) @symbol.closure)
  ] @visibility.internal
  (#eq? @_all "__all__")
  (#not-match? @symbol.name "^_"))

; Functions and lambdas bound in a function body are reachable only from inside it.
(function_definition
  body: (block
    [
      (function_definition
        name: (identifier) @symbol.name
        body: (block) @symbol.body) @symbol.function
      (decorated_definition
        definition: (function_definition
          name: (identifier) @symbol.name
          body: (block) @symbol.body) @symbol.function)
      (expression_statement
        (assignment
          left: (identifier) @symbol.name
          right: (lambda body: (_) @symbol.body)) @symbol.closure)
    ] @visibility.private))

; Methods, including static and class methods.
(class_definition
  body: (block
    [
      (function_definition
        name: (identifier) @symbol.name
        body: (block) @symbol.body) @symbol.method
      (decorated_definition
        definition: (function_definition
          name: (identifier) @symbol.name
          body: (block) @symbol.body) @symbol.method)
    ]))

(function_definition
  name: (identifier) @symbol.name
  body: (block) @symbol.body) @symbol.function

(assignment
  left: (identifier) @symbol.name
  right: (lambda body: (_) @symbol.body)) @symbol.closure

; Classes namespace their methods and nested classes.
(class_definition
  name: (identifier) @scope.name) @scope
