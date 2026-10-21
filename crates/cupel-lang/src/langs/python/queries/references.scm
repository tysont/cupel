; Python references (ADR-135). An attribute (`x.f`, `module.f`) and a dotted module path are
; members; the last name a `from` import imports is the name it binds. Decorators are attributes
; on the definition they decorate. `getattr` and its kin with a name that is not a string literal,
; `globals`, `locals`, `vars`, `eval`, `exec`, `__import__`, and `import_module` reach any name.

(import_from_statement
  name: (dotted_name
    (identifier) @reference.name .))

(import_from_statement
  name: (aliased_import
    name: (dotted_name
      (identifier) @reference.name .)))

(dotted_name
  (identifier) @reference.member)

(attribute
  attribute: (identifier) @reference.member)

(identifier) @reference.name

(string_content) @reference.text

(decorator
  [
    (identifier) @attribute.name
    (attribute
      attribute: (identifier) @attribute.name)
    (call
      function: [
        (identifier) @attribute.name
        (attribute
          attribute: (identifier) @attribute.name)
      ])
  ]) @reference.attribute

(call
  function: (identifier) @_function
  arguments: (argument_list
    .
    (_)
    .
    (_) @_name)
  (#any-of? @_function "getattr" "hasattr" "setattr" "delattr")
  (#not-match? @_name "^[\"']")) @dynamic.names

(call
  function: (identifier) @_function
  (#any-of? @_function "globals" "locals" "vars" "eval" "exec" "__import__" "import_module")) @dynamic.names

(call
  function: (attribute
    attribute: (identifier) @_function)
  (#eq? @_function "import_module")) @dynamic.names
