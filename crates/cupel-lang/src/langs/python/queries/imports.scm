; Python imports: one match per imported module (`import a.b, c`) and per imported name
; (`from .a import b, c`); the resolver joins a `from` import's module and name. Relative
; modules keep their leading dots. `from __future__` is a compiler directive, not an import.
; `from .a import b as c` binds `b` under the local name `c`.

(import_statement
  name: [
    (dotted_name) @import.specifier
    (aliased_import name: (dotted_name) @import.specifier)
  ]) @import

(import_from_statement
  module_name: (_) @import.specifier
  name: [
    (dotted_name) @import.name
    (aliased_import
      name: (dotted_name) @import.name
      alias: (identifier) @import.alias)
  ]) @import

(import_from_statement
  module_name: (_) @import.specifier
  (wildcard_import) @import.name) @import

; `__all__` lists the names a module exports; an import of one of them is a re-export.
(module
  (expression_statement
    (assignment
      left: (identifier) @_all
      right: [
        (list (string (string_content) @reexport.name))
        (tuple (string (string_content) @reexport.name))
      ]))
  (#eq? @_all "__all__"))

; An import under `if TYPE_CHECKING:` or `if typing.TYPE_CHECKING:` runs only for a type checker,
; never at run time: an import of types alone (ADR-211). Its `else:` branch runs.
(if_statement
  condition: [
    (identifier) @_guard
    (attribute attribute: (identifier) @_guard)
  ]
  consequence: (block
    [
      (import_statement)
      (import_from_statement)
    ] @import.type)
  (#eq? @_guard "TYPE_CHECKING"))
