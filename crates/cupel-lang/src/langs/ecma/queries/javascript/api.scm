; JavaScript-only API patterns: their parameter shapes are impossible in TypeScript's grammar,
; which wraps every parameter in `required_parameter` or `optional_parameter`.

; Methods of exported classes, except `#private` ones.
(export_statement
  declaration: (class_declaration
    body: (class_body
      (method_definition
        name: (property_identifier) @api.name
        parameters: (formal_parameters) @api.signature) @api.method)))

; Parameter names and destructuring patterns; a default value stays in the signature.
(formal_parameters
  [
    (identifier)
    (object_pattern)
    (array_pattern)
  ] @api.local)

(formal_parameters
  (assignment_pattern
    left: (_) @api.local))
