; Go imports: one specifier per import spec, with or without a name (`u "a/b"`, `_ "embed"`,
; `. "a/b"`). The resolver maps each import path to the files of the package it names.

(import_spec
  path: [
    (interpreted_string_literal)
    (raw_string_literal)
  ] @import.specifier) @import
