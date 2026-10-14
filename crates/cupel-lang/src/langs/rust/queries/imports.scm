; Rust imports: `use` trees (expanded by the resolver), child-module declarations, and
; `extern crate`. A `use` with any visibility (`pub use`, `pub(crate) use`) re-exports what it
; imports. Inline modules are scopes, so imports inside them resolve from there.

(use_declaration
  (visibility_modifier)? @import.reexport
  argument: (_) @import.specifier) @import

(mod_item
  name: (identifier) @import.specifier
  !body) @import @import.module

(extern_crate_declaration
  name: (identifier) @import.specifier) @import

(mod_item
  name: (identifier) @scope.name
  body: (declaration_list)) @scope
