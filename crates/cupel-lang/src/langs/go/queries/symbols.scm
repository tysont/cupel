; Go symbols: functions, methods, and function literals bound to names. The package clause names
; the namespace; a method's receiver type is a namespace segment captured in its own match.

(package_clause
  (package_identifier) @namespace.name)

(method_declaration
  receiver: (parameter_list
    (parameter_declaration
      type: [
        (type_identifier) @scope.name
        (pointer_type (type_identifier) @scope.name)
        (generic_type type: (type_identifier) @scope.name)
        (pointer_type (generic_type type: (type_identifier) @scope.name))
      ]))
  name: (field_identifier) @symbol.name
  body: (block) @symbol.body) @symbol.method

(function_declaration
  name: (identifier) @symbol.name
  body: (block) @symbol.body) @symbol.function

(short_var_declaration
  left: (expression_list . (identifier) @symbol.name .)
  right: (expression_list . (func_literal body: (block) @symbol.body) .)) @symbol.closure

(var_spec
  name: (identifier) @symbol.name
  value: (expression_list . (func_literal body: (block) @symbol.body) .)) @symbol.closure

; Parameters: each name of a declaration in a function's own parameter list (`a, b int` is two),
; or the declaration itself when it names none (`func(int)`). The receiver list is not matched.
(function_declaration
  parameters: (parameter_list
    (parameter_declaration
      name: (identifier) @symbol.parameter)))

(method_declaration
  parameters: (parameter_list
    (parameter_declaration
      name: (identifier) @symbol.parameter)))

(func_literal
  parameters: (parameter_list
    (parameter_declaration
      name: (identifier) @symbol.parameter)))

(function_declaration
  parameters: (parameter_list
    (parameter_declaration
      !name) @symbol.parameter))

(method_declaration
  parameters: (parameter_list
    (parameter_declaration
      !name) @symbol.parameter))

(func_literal
  parameters: (parameter_list
    (parameter_declaration
      !name) @symbol.parameter))

(parameter_list
  (variadic_parameter_declaration) @symbol.parameter)
