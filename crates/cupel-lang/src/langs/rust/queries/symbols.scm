; Rust symbols: functions, methods in impl blocks and traits, and closures bound by `let`.
; The earliest matching pattern wins for a definition, so trait-impl methods come first.

; Methods implementing a trait are reachable through the trait: public.
(impl_item
  trait: (_)
  body: (declaration_list
    (function_item
      name: (identifier) @symbol.name
      body: (block) @symbol.body) @symbol.method @visibility.public))

; Inherent methods and associated functions.
(impl_item
  body: (declaration_list
    (function_item
      (visibility_modifier)? @symbol.visibility
      name: (identifier) @symbol.name
      body: (block) @symbol.body) @symbol.method))

; Default methods of a trait are part of the trait: public.
(trait_item
  body: (declaration_list
    (function_item
      name: (identifier) @symbol.name
      body: (block) @symbol.body) @symbol.method @visibility.public))

; Every other function, including functions nested in function bodies.
(function_item
  (visibility_modifier)? @symbol.visibility
  name: (identifier) @symbol.name
  body: (block) @symbol.body) @symbol.function

; Closures bound to a name.
(let_declaration
  pattern: (identifier) @symbol.name
  value: (closure_expression
    body: (_) @symbol.body)) @symbol.closure

; Namespaces: inline modules, impl blocks (by their type), and traits.
(mod_item
  name: (identifier) @scope.name
  body: (declaration_list)) @scope

(impl_item
  type: [
    (type_identifier) @scope.name
    (generic_type type: (type_identifier) @scope.name)
    (scoped_type_identifier name: (type_identifier) @scope.name)
  ]) @scope

(trait_item
  name: (type_identifier) @scope.name) @scope

; Parameters: each declared parameter of a function or closure. `self` is a receiver, so
; `self_parameter` is never captured.
(parameters
  (parameter) @symbol.parameter)

(closure_parameters
  (_) @symbol.parameter)
