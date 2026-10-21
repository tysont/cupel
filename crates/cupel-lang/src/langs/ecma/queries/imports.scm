; JavaScript and TypeScript imports: ESM `import … from`, side-effect imports, re-exports
; (`export … from`), CommonJS `require("…")`, and dynamic `import("…")`. Only string
; literals are specifiers; computed ones cannot be resolved. A default import imports `default`;
; named imports and re-exports name what they take as the module spells it.

(import_statement
  (import_clause
    (identifier)? @import.default
    (named_imports)? @import.names)?
  source: (string (string_fragment) @import.specifier)) @import

(export_statement
  (export_clause)? @import.names
  source: (string (string_fragment) @import.specifier)) @import @import.reexport

(call_expression
  function: (identifier) @_require
  arguments: (arguments
    .
    (string (string_fragment) @import.specifier)
    .)
  (#eq? @_require "require")) @import

(call_expression
  function: (import)
  arguments: (arguments
    .
    (string (string_fragment) @import.specifier))) @import
