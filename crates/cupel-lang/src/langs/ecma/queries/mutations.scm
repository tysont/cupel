; JavaScript and TypeScript mutation sites. Operators are captured as their token, literals as
; the literal, and the extended set captures whole statements, returned values, and conditions.
; Strict (`===`) and loose (`==`) equality are both equality sites; `??` is not a site.

(binary_expression operator: ["<" "<=" ">" ">="] @mut.relational)
(binary_expression operator: ["===" "!==" "==" "!="] @mut.equality)
(binary_expression operator: ["+" "-"] @mut.arith-add-sub)
(augmented_assignment_expression operator: ["+=" "-="] @mut.arith-add-sub)
(binary_expression operator: ["*" "/"] @mut.arith-mul-div)
(augmented_assignment_expression operator: ["*=" "/="] @mut.arith-mul-div)
(binary_expression operator: ["&&" "||"] @mut.boolean-op)

[
  (true)
  (false)
] @mut.boolean-literal

((number) @mut.zero-one
  (#any-of? @mut.zero-one "0" "1"))

; Extended set. Only statements directly in a block are deleted, so `if (c) f();` keeps a
; statement after its condition.
(statement_block
  (expression_statement
    [
      (call_expression)
      (await_expression)
      (assignment_expression)
      (augmented_assignment_expression)
      (update_expression)
    ]) @mut.statement-deletion)

(return_statement (_) @mut.return-value)

((number) @mut.constant
  (#match? @mut.constant "^[0-9]+$")
  (#not-any-of? @mut.constant "0" "1"))

(binary_expression
  operator: ["<" "<=" ">" ">="]
  right: (number) @mut.boundary
  (#match? @mut.boundary "^[0-9]+$"))

(if_statement
  condition: (parenthesized_expression (_) @mut.negation))

(while_statement
  condition: (parenthesized_expression (_) @mut.negation))
