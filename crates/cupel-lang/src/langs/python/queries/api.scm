; Python public API (ADR-034): module-level functions, classes (Protocol and ABC subclasses are
; interfaces), `type` aliases, upper-case constants, and variables, plus the public methods and
; `__init__` of public classes. The earliest pattern wins for an item node, and an item whose
; winning match has no `@api.name` is not reported, so the patterns come in three tiers: names a
; module-level `__all__` lists; then, when `__all__` exists, every other definition, unnamed and
; therefore hidden; then, without `__all__`, every name without a leading underscore.
; Parameter names are local (`_`), except `self` and `cls`.

; Tier 1: listed in `__all__` (assigned before the definitions, as PEP 8 places it).
(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [
        (list (string (string_content) @_exported))
        (tuple (string (string_content) @_exported))
      ]))
  [
    (class_definition
      name: (identifier) @api.name
      type_parameters: (_)? @api.signature
      superclasses: (argument_list
        [
          (identifier) @_base
          (attribute attribute: (identifier) @_base)
        ]) @api.signature) @api.interface
    (decorated_definition
      definition: (class_definition
        name: (identifier) @api.name
        type_parameters: (_)? @api.signature
        superclasses: (argument_list
          [
            (identifier) @_base
            (attribute attribute: (identifier) @_base)
          ]) @api.signature) @api.interface)
  ]
  (#eq? @_all "__all__")
  (#eq? @_exported @api.name)
  (#any-of? @_base "Protocol" "ABC"))

(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [
        (list (string (string_content) @_exported))
        (tuple (string (string_content) @_exported))
      ]))
  [
    (class_definition
      name: (identifier) @api.name
      type_parameters: (_)? @api.signature
      superclasses: (argument_list)? @api.signature) @api.type
    (decorated_definition
      definition: (class_definition
        name: (identifier) @api.name
        type_parameters: (_)? @api.signature
        superclasses: (argument_list)? @api.signature) @api.type)
    (type_alias_statement
      left: (type (identifier) @api.name)
      right: (type) @api.signature) @api.type
  ]
  (#eq? @_all "__all__")
  (#eq? @_exported @api.name))

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
      name: (identifier) @api.name
      type_parameters: (_)? @api.signature
      parameters: (parameters) @api.signature
      return_type: (type)? @api.signature) @api.function
    (decorated_definition
      definition: (function_definition
        name: (identifier) @api.name
        type_parameters: (_)? @api.signature
        parameters: (parameters) @api.signature
        return_type: (type)? @api.signature) @api.function)
    (expression_statement
      (assignment
        left: (identifier) @api.name
        right: (lambda (lambda_parameters)? @api.signature)) @api.function)
  ]
  (#eq? @_all "__all__")
  (#eq? @_exported @api.name))

(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [
        (list (string (string_content) @_exported))
        (tuple (string (string_content) @_exported))
      ]))
  (expression_statement
    (assignment
      left: (identifier) @api.name
      type: (type)? @api.signature) @api.constant)
  (#eq? @_all "__all__")
  (#eq? @_exported @api.name)
  (#match? @api.name "^[A-Z][A-Z0-9_]*$"))

(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [
        (list (string (string_content) @_exported))
        (tuple (string (string_content) @_exported))
      ]))
  (expression_statement
    (assignment
      left: (identifier) @api.name
      type: (type)? @api.signature) @api.variable)
  (#eq? @_all "__all__")
  (#eq? @_exported @api.name))

; Methods of a class listed in `__all__`.
(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [
        (list (string (string_content) @_exported))
        (tuple (string (string_content) @_exported))
      ]))
  [
    (class_definition
      name: (identifier) @_class
      body: (block
        [
          (function_definition
            name: (identifier) @api.name
            type_parameters: (_)? @api.signature
            parameters: (parameters) @api.signature
            return_type: (type)? @api.signature) @api.method
          (decorated_definition
            definition: (function_definition
              name: (identifier) @api.name
              type_parameters: (_)? @api.signature
              parameters: (parameters) @api.signature
              return_type: (type)? @api.signature) @api.method)
        ]))
    (decorated_definition
      definition: (class_definition
        name: (identifier) @_class
        body: (block
          [
            (function_definition
              name: (identifier) @api.name
              type_parameters: (_)? @api.signature
              parameters: (parameters) @api.signature
              return_type: (type)? @api.signature) @api.method
            (decorated_definition
              definition: (function_definition
                name: (identifier) @api.name
                type_parameters: (_)? @api.signature
                parameters: (parameters) @api.signature
                return_type: (type)? @api.signature) @api.method)
          ])))
  ]
  (#eq? @_all "__all__")
  (#eq? @_exported @_class)
  (#match? @api.name "^([^_].*|__init__)$"))

; Tier 2: with `__all__`, every other module-level definition and method is hidden.
(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [(list) (tuple)]))
  [
    (function_definition) @api.function
    (class_definition) @api.type
    (type_alias_statement) @api.type
    (decorated_definition
      definition: [
        (function_definition) @api.function
        (class_definition) @api.type
      ])
    (expression_statement (assignment) @api.variable)
  ]
  (#eq? @_all "__all__"))

(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [(list) (tuple)]))
  [
    (class_definition
      body: (block
        [
          (function_definition) @api.method
          (decorated_definition definition: (function_definition) @api.method)
        ]))
    (decorated_definition
      definition: (class_definition
        body: (block
          [
            (function_definition) @api.method
            (decorated_definition definition: (function_definition) @api.method)
          ])))
  ]
  (#eq? @_all "__all__"))

; Tier 3: without `__all__`, names without a leading underscore.
(module
  [
    (class_definition
      name: (identifier) @api.name
      type_parameters: (_)? @api.signature
      superclasses: (argument_list
        [
          (identifier) @_base
          (attribute attribute: (identifier) @_base)
        ]) @api.signature) @api.interface
    (decorated_definition
      definition: (class_definition
        name: (identifier) @api.name
        type_parameters: (_)? @api.signature
        superclasses: (argument_list
          [
            (identifier) @_base
            (attribute attribute: (identifier) @_base)
          ]) @api.signature) @api.interface)
  ]
  (#not-match? @api.name "^_")
  (#any-of? @_base "Protocol" "ABC"))

(module
  [
    (class_definition
      name: (identifier) @api.name
      type_parameters: (_)? @api.signature
      superclasses: (argument_list)? @api.signature) @api.type
    (decorated_definition
      definition: (class_definition
        name: (identifier) @api.name
        type_parameters: (_)? @api.signature
        superclasses: (argument_list)? @api.signature) @api.type)
    (type_alias_statement
      left: (type (identifier) @api.name)
      right: (type) @api.signature) @api.type
  ]
  (#not-match? @api.name "^_"))

(module
  [
    (function_definition
      name: (identifier) @api.name
      type_parameters: (_)? @api.signature
      parameters: (parameters) @api.signature
      return_type: (type)? @api.signature) @api.function
    (decorated_definition
      definition: (function_definition
        name: (identifier) @api.name
        type_parameters: (_)? @api.signature
        parameters: (parameters) @api.signature
        return_type: (type)? @api.signature) @api.function)
    (expression_statement
      (assignment
        left: (identifier) @api.name
        right: (lambda (lambda_parameters)? @api.signature)) @api.function)
  ]
  (#not-match? @api.name "^_"))

(module
  (expression_statement
    (assignment
      left: (identifier) @api.name
      type: (type)? @api.signature) @api.constant)
  (#match? @api.name "^[A-Z][A-Z0-9_]*$"))

(module
  (expression_statement
    (assignment
      left: (identifier) @api.name
      type: (type)? @api.signature) @api.variable)
  (#not-match? @api.name "^_"))

(module
  [
    (class_definition
      name: (identifier) @_class
      body: (block
        [
          (function_definition
            name: (identifier) @api.name
            type_parameters: (_)? @api.signature
            parameters: (parameters) @api.signature
            return_type: (type)? @api.signature) @api.method
          (decorated_definition
            definition: (function_definition
              name: (identifier) @api.name
              type_parameters: (_)? @api.signature
              parameters: (parameters) @api.signature
              return_type: (type)? @api.signature) @api.method)
        ]))
    (decorated_definition
      definition: (class_definition
        name: (identifier) @_class
        body: (block
          [
            (function_definition
              name: (identifier) @api.name
              type_parameters: (_)? @api.signature
              parameters: (parameters) @api.signature
              return_type: (type)? @api.signature) @api.method
            (decorated_definition
              definition: (function_definition
                name: (identifier) @api.name
                type_parameters: (_)? @api.signature
                parameters: (parameters) @api.signature
                return_type: (type)? @api.signature) @api.method)
          ])))
  ]
  (#not-match? @_class "^_")
  (#match? @api.name "^([^_].*|__init__)$"))

; Classes namespace their methods.
(class_definition
  name: (identifier) @scope.name) @scope

; Parameter names are local; `self` and `cls` say what kind of method it is.
(parameters
  (identifier) @api.local
  (#not-any-of? @api.local "self" "cls"))

(typed_parameter
  .
  (identifier) @api.local
  (#not-any-of? @api.local "self" "cls"))

(default_parameter name: (identifier) @api.local)
(typed_default_parameter name: (identifier) @api.local)
(list_splat_pattern (identifier) @api.local)
(dictionary_splat_pattern (identifier) @api.local)
(lambda_parameters (identifier) @api.local)

; Roots: functions the `if __name__ == "__main__":` guard calls, directly or as an argument
; (`sys.exit(main())`).
(module
  [
    (function_definition
      name: (identifier) @_function) @root.entry-point
    (decorated_definition
      definition: (function_definition
        name: (identifier) @_function) @root.entry-point)
  ]
  (if_statement
    condition: (comparison_operator
      (identifier) @_name
      (string (string_content) @_main))
    consequence: (block
      (expression_statement
        [
          (call function: (identifier) @_called)
          (call
            arguments: (argument_list
              (call function: (identifier) @_called)))
        ])))
  (#eq? @_name "__name__")
  (#eq? @_main "__main__")
  (#eq? @_function @_called))

; Roots: functions the module's own top-level statements call, directly, as an argument, or as a
; keyword argument's value, alone or as an assignment's value: a module runs that code whenever it
; runs, so for a module nothing imports (a setup script setuptools runs) it is the script's body,
; as a `__main__` guard's is (ADR-138 §9).
(module
  [
    (function_definition
      name: (identifier) @_function) @root.entry-point
    (decorated_definition
      definition: (function_definition
        name: (identifier) @_function) @root.entry-point)
  ]
  (expression_statement
    [
      (call function: (identifier) @_called)
      (call
        arguments: (argument_list
          [
            (call function: (identifier) @_called)
            (keyword_argument value: (call function: (identifier) @_called))
          ]))
      (assignment right: (call function: (identifier) @_called))
      (assignment
        right: (call
          arguments: (argument_list
            [
              (call function: (identifier) @_called)
              (keyword_argument value: (call function: (identifier) @_called))
            ])))
    ])
  (#eq? @_function @_called))

; Hooks: dunder methods the runtime calls, pytest hooks, and fixtures pytest injects.
(function_definition
  name: (identifier) @_dunder
  (#match? @_dunder "^__.+__$")) @root.hook

(function_definition
  name: (identifier) @_hook
  (#match? @_hook "^pytest_")) @root.hook

(decorated_definition
  (decorator
    [
      (identifier)
      (attribute)
      (call function: [(identifier) (attribute)])
    ] @_fixture)
  definition: (function_definition) @root.hook
  (#match? @_fixture "^(pytest\\.)?fixture\\b"))
