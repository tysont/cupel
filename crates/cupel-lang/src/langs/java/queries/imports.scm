; Java imports: one specifier per import declaration, the dotted name as written. A type-import-on-
; demand (`import a.b.*;`) imports `*` from its package or type; a static import names a member
; of a type. The resolver maps each name to the source file of the type it names.

(import_declaration
  [(identifier) (scoped_identifier)] @import.specifier
  (asterisk)? @import.name) @import
