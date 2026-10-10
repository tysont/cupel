; Go public API: package-level items whose names are capitalized, methods of capitalized types,
; and the capitalized methods of capitalized interfaces. A method's path includes its receiver
; type (an `@scope.name` in its own match) and its signature starts at the receiver, so changing
; a pointer receiver to a value receiver changes the API. Test functions are not API.

(method_declaration
  receiver: (parameter_list
    (parameter_declaration
      type: [
        (type_identifier) @scope.name
        (pointer_type (type_identifier) @scope.name)
        (generic_type type: (type_identifier) @scope.name)
        (pointer_type (generic_type type: (type_identifier) @scope.name))
      ])) @api.signature
  name: (field_identifier) @api.name
  parameters: (parameter_list) @api.signature
  result: (_)? @api.signature
  (#match? @scope.name "^[A-Z]")
  (#match? @api.name "^[A-Z]")) @api.method

(function_declaration
  name: (identifier) @api.name
  type_parameters: (type_parameter_list)? @api.signature
  parameters: (parameter_list) @api.signature
  result: (_)? @api.signature
  (#match? @api.name "^[A-Z]")
  (#not-match? @api.name "^(Test|Benchmark|Example|Fuzz)([^a-z]|$)")) @api.function

(source_file
  (type_declaration
    (type_spec
      name: (type_identifier) @api.name
      type_parameters: (type_parameter_list)? @api.signature
      type: (interface_type)
      (#match? @api.name "^[A-Z]")) @api.interface))

(source_file
  (type_declaration
    (type_spec
      name: (type_identifier) @_interface
      type: (interface_type
        (method_elem
          name: (field_identifier) @api.name
          parameters: (parameter_list) @api.signature
          result: (_)? @api.signature) @api.method))
    (#match? @_interface "^[A-Z]")
    (#match? @api.name "^[A-Z]")))

(source_file
  (type_declaration
    [
      (type_spec
        name: (type_identifier) @api.name
        type_parameters: (type_parameter_list)? @api.signature)
      (type_alias
        name: (type_identifier) @api.name)
    ] @api.type
    (#match? @api.name "^[A-Z]")))

(source_file
  (const_declaration
    (const_spec
      name: (identifier) @api.name
      type: (_)? @api.signature
      (#match? @api.name "^[A-Z]")) @api.constant))

(source_file
  (var_declaration
    [
      (var_spec
        name: (identifier) @api.name
        type: (_)? @api.signature) @api.variable
      (var_spec_list
        (var_spec
          name: (identifier) @api.name
          type: (_)? @api.signature) @api.variable)
    ]
    (#match? @api.name "^[A-Z]")))

[
  (parameter_declaration
    name: (identifier) @api.local)
  (variadic_parameter_declaration
    name: (identifier) @api.local)
]

; Roots: functions the go tool calls by name besides tests (`main`, `init`, and `TestMain` are
; named in tables.toml).
((function_declaration
  name: (identifier) @_hook) @root.hook
  (#match? @_hook "^(Benchmark|Example|Fuzz)([^a-z]|$)"))

; Namespaces: an interface scopes its methods.
(type_spec
  name: (type_identifier) @scope.name
  type: (interface_type)) @scope

(package_clause
  (package_identifier) @namespace.name)
