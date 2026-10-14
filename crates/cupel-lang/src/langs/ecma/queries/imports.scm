; JavaScript and TypeScript imports: ESM `import … from`, side-effect imports, re-exports
; (`export … from`), CommonJS `require("…")`, and dynamic `import("…")`. Only string
; literals are specifiers; computed ones cannot be resolved.

(import_statement
  source: (string (string_fragment) @import.specifier)) @import

(export_statement
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
