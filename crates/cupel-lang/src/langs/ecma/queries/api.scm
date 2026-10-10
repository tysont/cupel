; JavaScript and TypeScript public API: declarations exported in place, default-exported
; functions (named `default`), and namespace re-exports. Export clauses and named re-exports
; are not items (ADR-031). The language overlays add class methods, parameter names, and
; TypeScript's typed signatures and declarations; their patterns come first and win.

(export_statement
  declaration: [
    (function_declaration
      name: (identifier) @api.name
      parameters: (formal_parameters) @api.signature)
    (generator_function_declaration
      name: (identifier) @api.name
      parameters: (formal_parameters) @api.signature)
  ] @api.function)

(export_statement
  declaration: (class_declaration
    name: (_) @api.name) @api.type)

; A `const` bound to a function is a function; other `const`s are constants.
(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (identifier) @api.name
      value: [
        (arrow_function parameters: (formal_parameters) @api.signature)
        (arrow_function parameter: (identifier) @api.signature @api.local)
        (function_expression parameters: (formal_parameters) @api.signature)
      ]) @api.function))

(export_statement
  declaration: (lexical_declaration
    kind: "const"
    (variable_declarator
      name: (identifier) @api.name) @api.constant))

(export_statement
  declaration: [
    (lexical_declaration
      kind: "let"
      (variable_declarator
        name: (identifier) @api.name) @api.variable)
    (variable_declaration
      (variable_declarator
        name: (identifier) @api.name) @api.variable)
  ])

; Default-exported functions are called `default` and are framework hooks: bundlers, routers,
; and plugin hosts call a module's default export by convention.
(export_statement
  "default" @api.name
  value: [
    (function_expression parameters: (formal_parameters) @api.signature)
    (arrow_function parameters: (formal_parameters) @api.signature)
  ] @api.function @root.hook)

(export_statement
  "default" @api.name
  declaration: (function_declaration
    parameters: (formal_parameters) @api.signature) @api.function @root.hook)

; CommonJS: `exports.name = function …` and `module.exports.name = …`.
(assignment_expression
  left: (member_expression
    object: [
      (identifier) @_exports
      (member_expression
        object: (identifier) @_module
        property: (property_identifier) @_exports)
    ]
    property: (property_identifier) @api.name)
  right: [
    (function_expression parameters: (formal_parameters) @api.signature)
    (arrow_function parameters: (formal_parameters) @api.signature)
  ]
  (#eq? @_exports "exports")
  (#eq? @_module "module")) @api.function

; `export * as name from "…"` exports a module.
(export_statement
  (namespace_export (_) @api.name)) @api.module

; Rest parameters are local names wherever they appear.
(rest_pattern) @api.local

; Classes namespace their members.
(class_declaration
  name: (_) @scope.name) @scope
