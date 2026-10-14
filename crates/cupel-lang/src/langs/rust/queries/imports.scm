; Rust imports: `use` trees (expanded by the resolver), child-module declarations, and
; `extern crate`. Inline modules are scopes, so imports inside them resolve from there.

(use_declaration
  argument: (_) @import.specifier) @import

(mod_item
  name: (identifier) @import.specifier
  !body) @import @import.module

(extern_crate_declaration
  name: (identifier) @import.specifier) @import

(mod_item
  name: (identifier) @scope.name
  body: (declaration_list)) @scope
