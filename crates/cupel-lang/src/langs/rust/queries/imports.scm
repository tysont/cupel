; Rust imports: `use` trees (expanded by the resolver), child-module declarations, and
; `extern crate`. A `use` with any visibility (`pub use`, `pub(crate) use`) re-exports what it
; imports. Inline modules are scopes, so imports inside them resolve from there.

(use_declaration
  (visibility_modifier)? @import.reexport
  argument: (_) @import.specifier) @import

(mod_item
  name: (identifier) @import.specifier
  !body) @import @import.module

; `#[path = "core.rs"] mod imp;` loads the file the attribute names.
((attribute_item
  (attribute
    (identifier) @_path
    value: (string_literal) @import.path))
  .
  (mod_item
    name: (identifier) @import.specifier
    !body) @import @import.module
  (#eq? @_path "path"))

(extern_crate_declaration
  name: (identifier) @import.specifier) @import

(mod_item
  name: (identifier) @scope.name
  body: (declaration_list)) @scope
