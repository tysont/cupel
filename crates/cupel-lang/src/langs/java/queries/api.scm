; Java public API: public types; the public and protected methods, constructors, and fields of
; public classes, enums, and records; the methods and constants of public interfaces, which are
; public unless `private`; the constants of public enums; and the module a `module-info.java`
; declares. A method's signature runs from its type parameters or result type through its
; `throws` clause, so it keeps its name and parameter types; `static` and annotations are left
; out. Parameter names are local.

(package_declaration
  [(identifier) (scoped_identifier)] @namespace.name)

[
  (class_declaration name: (identifier) @scope.name)
  (interface_declaration name: (identifier) @scope.name)
  (enum_declaration name: (identifier) @scope.name)
  (record_declaration name: (identifier) @scope.name)
  (annotation_type_declaration name: (identifier) @scope.name)
] @scope

; Methods and constructors of public classes, enums, and records.
(_
  (modifiers "public")
  body: [
    (class_body
      (method_declaration
        (modifiers ["public" "protected"])
        type_parameters: (_)? @api.signature
        type: (_) @api.signature
        name: (identifier) @api.name
        parameters: (formal_parameters) @api.signature
        (throws)? @api.signature) @api.method)
    (enum_body
      (enum_body_declarations
        (method_declaration
          (modifiers ["public" "protected"])
          type_parameters: (_)? @api.signature
          type: (_) @api.signature
          name: (identifier) @api.name
          parameters: (formal_parameters) @api.signature
          (throws)? @api.signature) @api.method))
  ])

(_
  (modifiers "public")
  body: (class_body
    (constructor_declaration
      (modifiers ["public" "protected"])
      type_parameters: (_)? @api.signature
      name: (identifier) @api.name
      parameters: (formal_parameters) @api.signature
      (throws)? @api.signature) @api.method))

; Methods of public interfaces, with or without a body, unless `private` (the method's text
; starts with its annotations, then its modifiers).
(interface_declaration
  (modifiers "public")
  body: (interface_body
    (method_declaration
      type_parameters: (_)? @api.signature
      type: (_) @api.signature
      name: (identifier) @api.name
      parameters: (formal_parameters) @api.signature
      (throws)? @api.signature) @api.method)
  (#not-match? @api.method "^(\\s|@[^\\n]*\\n)*private\\s"))

; Public types.
(interface_declaration
  (modifiers "public")
  name: (identifier) @api.name
  type_parameters: (_)? @api.signature
  (extends_interfaces)? @api.signature) @api.interface

(annotation_type_declaration
  (modifiers "public")
  name: (identifier) @api.name) @api.interface

(class_declaration
  (modifiers "public")
  name: (identifier) @api.name
  type_parameters: (_)? @api.signature
  superclass: (_)? @api.signature
  interfaces: (_)? @api.signature) @api.type

(enum_declaration
  (modifiers "public")
  name: (identifier) @api.name
  interfaces: (_)? @api.signature) @api.type

(record_declaration
  (modifiers "public")
  name: (identifier) @api.name
  type_parameters: (_)? @api.signature
  parameters: (formal_parameters) @api.signature
  interfaces: (_)? @api.signature) @api.type

; Fields of public classes: `static final` ones are constants, the rest variables.
(_
  (modifiers "public")
  body: (class_body
    (field_declaration
      (modifiers "public" "static" "final")
      type: (_) @api.signature
      declarator: (variable_declarator
        name: (identifier) @api.name)) @api.constant))

(_
  (modifiers "public")
  body: (class_body
    (field_declaration
      (modifiers ["public" "protected"])
      type: (_) @api.signature
      declarator: (variable_declarator
        name: (identifier) @api.name)) @api.variable))

(interface_declaration
  (modifiers "public")
  body: (interface_body
    (constant_declaration
      type: (_) @api.signature
      declarator: (variable_declarator
        name: (identifier) @api.name)) @api.constant))

(enum_declaration
  (modifiers "public")
  body: (enum_body
    (enum_constant
      name: (identifier) @api.name) @api.constant))

(module_declaration
  name: (_) @api.name) @api.module

[
  (formal_parameter
    name: (identifier) @api.local)
  (spread_parameter
    (variable_declarator
      name: (identifier) @api.local))
]

; Roots: `public static void main(String[])` starts a program; methods that override a
; supertype's are called through it; lifecycle and dependency-injection annotations, and the
; serialization hooks named in tables.toml, are called by a framework or the runtime.
(method_declaration
  (modifiers "public" "static")
  type: (void_type)
  name: (identifier) @_main
  parameters: (formal_parameters . (formal_parameter) .)
  (#eq? @_main "main")) @root.entry-point

(method_declaration
  (modifiers
    (marker_annotation
      name: (identifier) @_hook))
  (#any-of? @_hook "Override" "PostConstruct" "PreDestroy" "Bean" "BeforeEach" "BeforeAll" "AfterEach" "AfterAll" "Before" "BeforeClass" "After" "AfterClass")) @root.hook
