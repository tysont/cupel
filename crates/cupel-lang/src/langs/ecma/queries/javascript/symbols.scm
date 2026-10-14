; JavaScript-only symbol patterns: parameter forms the TypeScript grammars reject (they wrap every
; parameter in `required_parameter` or `optional_parameter`).

; Parameters: each declared parameter, plain, defaulted, rest, or destructured.
(formal_parameters
  [
    (identifier)
    (assignment_pattern)
    (rest_pattern)
    (object_pattern)
    (array_pattern)
  ] @symbol.parameter)
