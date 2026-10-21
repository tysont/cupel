; Rust references (ADR-135). A macro's token tree is unstructured, so its identifiers are members
; (`self.f` inside `assert!` is two identifiers); a `use` path's last segment is the name the
; import binds; other path segments, fields, and method names are members.

(token_tree
  (identifier) @reference.member)

(use_declaration
  argument: (scoped_identifier
    name: (identifier) @reference.name))

(use_declaration
  argument: (identifier) @reference.name)

(use_list
  (identifier) @reference.name)

(use_list
  (scoped_identifier
    name: (identifier) @reference.name))

(use_as_clause
  path: (scoped_identifier
    name: (identifier) @reference.name))

(scoped_identifier
  name: (identifier) @reference.member)

(field_identifier) @reference.member

(identifier) @reference.name

; `#[serde(default = "port")]`, `#[serde(skip_serializing_if = "Option::is_none")]`.
(string_content) @reference.text

(attribute_item
  (attribute
    [
      (identifier) @attribute.name
      (scoped_identifier
        name: (identifier) @attribute.name)
    ])) @reference.attribute
