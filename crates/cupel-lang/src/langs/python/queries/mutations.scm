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
