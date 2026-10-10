; Rust public API: items marked plain `pub` (`pub(crate)` and other restrictions are not public),
; methods of inherent impls, and every item of a public trait. Paths use the same scopes as
; symbols.scm. Parameter patterns are local names.

(impl_item
  !trait
  body: (declaration_list
    (function_item
      (visibility_modifier) @_pub
      (#eq? @_pub "pub")
      name: (identifier) @api.name
      type_parameters: (type_parameters)? @api.signature
      parameters: (parameters) @api.signature
      return_type: (_)? @api.signature
      (where_clause)? @api.signature) @api.method))

(trait_item
  (visibility_modifier) @_pub
  (#eq? @_pub "pub")
  body: (declaration_list
    [
      (function_item
        name: (identifier) @api.name
        type_parameters: (type_parameters)? @api.signature
        parameters: (parameters) @api.signature
        return_type: (_)? @api.signature
        (where_clause)? @api.signature) @api.method
      (function_signature_item
        name: (identifier) @api.name
        type_parameters: (type_parameters)? @api.signature
        parameters: (parameters) @api.signature
        return_type: (_)? @api.signature
        (where_clause)? @api.signature) @api.method
    ]))

(function_item
  (visibility_modifier) @_pub
  (#eq? @_pub "pub")
  name: (identifier) @api.name
  type_parameters: (type_parameters)? @api.signature
  parameters: (parameters) @api.signature
  return_type: (_)? @api.signature
  (where_clause)? @api.signature) @api.function

([
  (struct_item
    (visibility_modifier) @_pub
    name: (type_identifier) @api.name
    type_parameters: (type_parameters)? @api.signature)
  (enum_item
    (visibility_modifier) @_pub
    name: (type_identifier) @api.name
    type_parameters: (type_parameters)? @api.signature)
  (union_item
    (visibility_modifier) @_pub
    name: (type_identifier) @api.name
    type_parameters: (type_parameters)? @api.signature)
  (type_item
    (visibility_modifier) @_pub
    name: (type_identifier) @api.name
    type_parameters: (type_parameters)? @api.signature)
] @api.type
  (#eq? @_pub "pub"))

(trait_item
  (visibility_modifier) @_pub
  (#eq? @_pub "pub")
  name: (type_identifier) @api.name
  type_parameters: (type_parameters)? @api.signature) @api.interface

(const_item
  (visibility_modifier) @_pub
  (#eq? @_pub "pub")
  name: (identifier) @api.name
  type: (_) @api.signature) @api.constant

(static_item
  (visibility_modifier) @_pub
  (#eq? @_pub "pub")
  name: (identifier) @api.name
  type: (_) @api.signature) @api.variable

(mod_item
  (visibility_modifier) @_pub
  (#eq? @_pub "pub")
  name: (identifier) @api.name) @api.module

(parameter
  pattern: (_) @api.local)

; Roots: functions the linker or an async runtime calls.
((attribute_item
  (attribute (identifier) @_attr (#eq? @_attr "no_mangle")))
  .
  (attribute_item)*
  .
  (function_item) @root.hook)

((attribute_item
  (attribute
    (identifier) @_unsafe
    (#eq? @_unsafe "unsafe")
    arguments: (token_tree (identifier) @_attr (#eq? @_attr "no_mangle"))))
  .
  (attribute_item)*
  .
  (function_item) @root.hook)

((attribute_item
  (attribute
    (scoped_identifier name: (identifier) @_main)
    (#eq? @_main "main")))
  .
  (attribute_item)*
  .
  (function_item) @root.entry-point)

; Namespaces, as in symbols.scm.
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
