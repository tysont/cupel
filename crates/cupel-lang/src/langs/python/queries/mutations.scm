; Python mutation sites. Operators are captured as their token (`is not` is one token), literals as
; the literal, and the extended set captures whole statements, returned values, and conditions.
; `is`/`is not` are identity comparisons and sit with `==`/`!=` under equality.

(comparison_operator operators: ["<" "<=" ">" ">="] @mut.relational)
(comparison_operator operators: ["==" "!=" "is" "is not"] @mut.equality)
(binary_operator operator: ["+" "-"] @mut.arith-add-sub)
(augmented_assignment operator: ["+=" "-="] @mut.arith-add-sub)
(binary_operator operator: ["*" "/" "//"] @mut.arith-mul-div)
(augmented_assignment operator: ["*=" "/=" "//="] @mut.arith-mul-div)
(boolean_operator operator: ["and" "or"] @mut.boolean-op)

[
  (true)
  (false)
] @mut.boolean-literal

((integer) @mut.zero-one
  (#any-of? @mut.zero-one "0" "1"))

; Extended set. A deleted statement becomes `pass`, since an empty block does not parse.
(expression_statement
  [
    (call)
    (await)
    (assignment)
    (augmented_assignment)
  ]) @mut.statement-deletion

(return_statement (_) @mut.return-value)

((integer) @mut.constant
  (#not-any-of? @mut.constant "0" "1"))

(comparison_operator
  operators: ["<" "<=" ">" ">="]
  (integer) @mut.boundary)

(if_statement condition: (_) @mut.negation)
(elif_clause condition: (_) @mut.negation)
(while_statement condition: (_) @mut.negation)

; The `strings` group (extended, and off unless enabled; ADR-180). A non-empty string is a site
; where it is a value, never a docstring or a dictionary key. Prefixed empty strings (`b''`,
; `f""`) are empty too.
((assignment right: (string) @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
((pair value: (string) @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
((argument_list (string) @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
((keyword_argument value: (string) @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
((list (string) @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
((conditional_expression (string) @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
((binary_operator [left: (string) right: (string)] @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
((comparison_operator (string) @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
((return_statement (string) @mut.string-empty)
  (#not-match? @mut.string-empty "^[A-Za-z]*(\"\"|'')$"))
