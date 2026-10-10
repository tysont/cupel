; TypeScript-only API patterns: typed signatures (type parameters, parameter types, return
; types), interfaces, type aliases, enums, namespaces, abstract classes, and `export =`. They
; come before the shared patterns, so they win for the items they match.

(export_statement
  declaration: [
    (function_declaration
      name: (identifier) @api.name
      type_parameters: (type_parameters)? @api.signature
      parameters: (formal_parameters) @api.signature
      return_type: (_)? @api.signature)
    (generator_function_declaration
      name: (identifier) @api.name
      type_parameters: (type_parameters)? @api.signature
      parameters: (formal_parameters) @api.signature
      return_type: (_)? @api.signature)
  ] @api.function)

(export_statement
  declaration: (lexical_declaration
    (variable_declarator
      name: (identifier) @api.name
      value: [
        (arrow_function
          type_parameters: (type_parameters)? @api.signature
          parameters: (formal_parameters) @api.signature
          return_type: (_)? @api.signature)
        (function_expression
          type_parameters: (type_parameters)? @api.signature
          parameters: (formal_parameters) @api.signature
          return_type: (_)? @api.signature)
      ]) @api.function))

(export_statement
  declaration: (lexical_declaration
    kind: "const"
    (variable_declarator
      name: (identifier) @api.name
      type: (type_annotation (_) @api.signature)) @api.constant))

(export_statement
  declaration: (lexical_declaration
    kind: "let"
    (variable_declarator
      name: (identifier) @api.name
      type: (type_annotation (_) @api.signature)) @api.variable))

(export_statement
  "default" @api.name
  value: [
    (function_expression
      parameters: (formal_parameters) @api.signature
      return_type: (_)? @api.signature)
    (arrow_function
      parameters: (formal_parameters) @api.signature
      return_type: (_)? @api.signature)
  ] @api.function @root.hook)

; Classes, abstract or not, and their public methods. `private` and `protected` members and
; `#private` ones are not API.
(export_statement
  declaration: [
    (class_declaration
      name: (_) @api.name
      type_parameters: (type_parameters)? @api.signature)
    (abstract_class_declaration
      name: (_) @api.name
      type_parameters: (type_parameters)? @api.signature)
  ] @api.type)

(export_statement
  declaration: [
    (class_declaration
      body: (class_body
        [
          (method_definition
            (accessibility_modifier)? @_access
            name: (property_identifier) @api.name
            type_parameters: (type_parameters)? @api.signature
            parameters: (formal_parameters) @api.signature
            return_type: (_)? @api.signature)
          (abstract_method_signature
            (accessibility_modifier)? @_access
            name: (property_identifier) @api.name
            type_parameters: (type_parameters)? @api.signature
            parameters: (formal_parameters) @api.signature
            return_type: (_)? @api.signature)
        ] @api.method))
    (abstract_class_declaration
      body: (class_body
        [
          (method_definition
            (accessibility_modifier)? @_access
            name: (property_identifier) @api.name
            type_parameters: (type_parameters)? @api.signature
            parameters: (formal_parameters) @api.signature
            return_type: (_)? @api.signature)
          (abstract_method_signature
            (accessibility_modifier)? @_access
            name: (property_identifier) @api.name
            type_parameters: (type_parameters)? @api.signature
            parameters: (formal_parameters) @api.signature
            return_type: (_)? @api.signature)
        ] @api.method))
  ]
  (#not-any-of? @_access "private" "protected"))

(export_statement
  declaration: (interface_declaration
    name: (_) @api.name
    type_parameters: (type_parameters)? @api.signature) @api.interface)

(export_statement
  declaration: (interface_declaration
    body: (interface_body
      (method_signature
        name: (property_identifier) @api.name
        type_parameters: (type_parameters)? @api.signature
        parameters: (formal_parameters) @api.signature
        return_type: (_)? @api.signature) @api.method)))

(export_statement
  declaration: [
    (type_alias_declaration
      name: (_) @api.name
      type_parameters: (type_parameters)? @api.signature)
    (enum_declaration
      name: (_) @api.name)
  ] @api.type)

(export_statement
  declaration: (internal_module
    name: (_) @api.name) @api.module)

; `export = name` makes `name` the whole module.
(export_statement
  .
  "="
  .
  (identifier) @api.name) @api.module

; Parameter names.
(required_parameter
  pattern: (_) @api.local)

(optional_parameter
  pattern: (_) @api.local)

; Namespaces for members, as in symbols.scm.
(abstract_class_declaration
  name: (_) @scope.name) @scope

(interface_declaration
  name: (_) @scope.name) @scope

(internal_module
  name: (_) @scope.name) @scope
