; Rust mutation sites. Operators are captured as their token, literals as the literal, and the
; extended set captures whole statements, returned values, and conditions.

(binary_expression operator: ["<" "<=" ">" ">="] @mut.relational)
(binary_expression operator: ["==" "!="] @mut.equality)
(binary_expression operator: ["+" "-"] @mut.arith-add-sub)
(compound_assignment_expr operator: ["+=" "-="] @mut.arith-add-sub)
(binary_expression operator: ["*" "/"] @mut.arith-mul-div)
(compound_assignment_expr operator: ["*=" "/="] @mut.arith-mul-div)
(binary_expression operator: ["&&" "||"] @mut.boolean-op)
(boolean_literal) @mut.boolean-literal

((integer_literal) @mut.zero-one
  (#any-of? @mut.zero-one "0" "1"))

; Extended set.
(expression_statement
  [
    (call_expression)
    (macro_invocation)
    (assignment_expression)
    (compound_assignment_expr)
  ]) @mut.statement-deletion

(return_expression (_) @mut.return-value)

((integer_literal) @mut.constant
  (#not-any-of? @mut.constant "0" "1"))

(binary_expression
  operator: ["<" "<=" ">" ">="]
  right: (integer_literal) @mut.boundary)

(if_expression
  condition: [
    (binary_expression)
    (unary_expression)
    (identifier)
    (call_expression)
    (field_expression)
    (parenthesized_expression)
  ] @mut.negation)

(while_expression
  condition: [
    (binary_expression)
    (unary_expression)
    (identifier)
    (call_expression)
    (field_expression)
    (parenthesized_expression)
  ] @mut.negation)
