; Go references (ADR-135). A selector's field (`pkg.F`, `x.m`) and every other field identifier
; (struct fields, an interface's method names, which a method satisfies by name) are members.
; `reflect` reaches only exported names, which are roots, so nothing is a dynamic access.

(selector_expression
  field: (field_identifier) @reference.member)

(field_identifier) @reference.member

(identifier) @reference.name

(interpreted_string_literal_content) @reference.text

(raw_string_literal) @reference.text
