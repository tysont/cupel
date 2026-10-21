; JavaScript and TypeScript references (ADR-135). A property (`x.f`, an object key, a class
; member) is a member; a shorthand property (`{ f }`) and an imported name are plain names.
; Decorators are attributes on the member they decorate. A subscript whose index is not a
; literal reaches members; `eval` and `Function` reach any name.

(member_expression
  property: [
    (property_identifier)
    (private_property_identifier)
  ] @reference.member)

(property_identifier) @reference.member

(private_property_identifier) @reference.member

(shorthand_property_identifier) @reference.name

(identifier) @reference.name

(string_fragment) @reference.text

(decorator
  [
    (identifier) @attribute.name
    (member_expression
      property: (property_identifier) @attribute.name)
    (call_expression
      function: [
        (identifier) @attribute.name
        (member_expression
          property: (property_identifier) @attribute.name)
      ])
  ]) @reference.attribute

(subscript_expression
  index: (_) @_index
  (#not-match? @_index "^([\"'`0-9-])")) @dynamic.members

(call_expression
  function: (identifier) @_function
  (#any-of? @_function "eval" "Function")) @dynamic.names

(new_expression
  constructor: (identifier) @_function
  (#eq? @_function "Function")) @dynamic.names
