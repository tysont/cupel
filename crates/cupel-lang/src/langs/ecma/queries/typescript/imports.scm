; TypeScript-only imports: `import name = require("…")`.

(import_statement
  (import_require_clause
    source: (string (string_fragment) @import.specifier))) @import

; Imports of types alone, which the compiler erases (ADR-165): `import type { A } from "…"`,
; `export type { A } from "…"`, and each named import marked `type` (`{ type A }`).
(import_statement "type") @import.type

(export_statement "type" source: (_)) @import.type

(import_specifier "type") @import.typed
