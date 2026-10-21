; Java references (ADR-135). A method called on an object or type, a field access, a method
; reference's method, and a qualified name's last segment are members; a type's name (`new
; Ledger()`) names its constructors. Annotations on a method or constructor are attributes.
; Reflection that looks a member up by name reaches methods, and every Java symbol is one.

(method_invocation
  object: (_)
  name: (identifier) @reference.member)

(field_access
  field: (identifier) @reference.member)

(method_reference
  (identifier) @reference.member .)

(scoped_identifier
  name: (identifier) @reference.member)

(identifier) @reference.name

(type_identifier) @reference.name

(string_fragment) @reference.text

(method_declaration
  (modifiers
    [
      (marker_annotation
        name: (_) @attribute.name)
      (annotation
        name: (_) @attribute.name)
    ] @reference.attribute))

(constructor_declaration
  (modifiers
    [
      (marker_annotation
        name: (_) @attribute.name)
      (annotation
        name: (_) @attribute.name)
    ] @reference.attribute))

(method_invocation
  name: (identifier) @_method
  (#any-of? @_method "getMethod" "getDeclaredMethod" "getMethods" "getDeclaredMethods" "forName")) @dynamic.members
