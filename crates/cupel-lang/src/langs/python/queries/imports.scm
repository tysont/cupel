; Python imports: one match per imported module (`import a.b, c`) and per imported name
; (`from .a import b, c`); the resolver joins a `from` import's module and name. Relative
; modules keep their leading dots. `from __future__` is a compiler directive, not an import.

(import_statement
  name: [
    (dotted_name) @import.specifier
    (aliased_import name: (dotted_name) @import.specifier)
  ]) @import

(import_from_statement
  module_name: (_) @import.specifier
  name: [
    (dotted_name) @import.name
    (aliased_import name: (dotted_name) @import.name)
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
