; Java mutation sites. Operators are captured as their token, literals as the literal, and the
; extended set captures whole statements, returned values, and conditions.

(binary_expression operator: ["<" "<=" ">" ">="] @mut.relational)
(binary_expression operator: ["==" "!="] @mut.equality)
(binary_expression operator: ["+" "-"] @mut.arith-add-sub)
(assignment_expression operator: ["+=" "-="] @mut.arith-add-sub)
(update_expression ["++" "--"] @mut.arith-add-sub)
(binary_expression operator: ["*" "/"] @mut.arith-mul-div)
(assignment_expression operator: ["*=" "/="] @mut.arith-mul-div)
(binary_expression operator: ["&&" "||"] @mut.boolean-op)
[(true) (false)] @mut.boolean-literal

((decimal_integer_literal) @mut.zero-one
  (#any-of? @mut.zero-one "0" "1"))

; Extended set. Only statements directly in a block or a switch group are deleted: calls,
; assignments, and increments evaluated for their effect, never a `for` clause's update.
(block
  (expression_statement
    [
      (method_invocation)
      (assignment_expression)
      (update_expression)
    ]) @mut.statement-deletion)

(switch_block_statement_group
  (expression_statement
    [
      (method_invocation)
      (assignment_expression)
      (update_expression)
    ]) @mut.statement-deletion)

; Each returned value is replaced by `null`. Literals are left to the literal operators, and
; arithmetic and comparisons, which are never references, are not captured; a mutant returning
; `null` from a method of primitive type does not compile, and the engine reports it so.
(return_statement
  [
    (identifier)
    (method_invocation)
    (field_access)
    (array_access)
    (object_creation_expression)
    (array_creation_expression)
    (cast_expression)
    (lambda_expression)
    (method_reference)
    (parenthesized_expression)
    (ternary_expression)
  ] @mut.return-value)

((decimal_integer_literal) @mut.constant
  (#not-any-of? @mut.constant "0" "1"))

(binary_expression
  operator: ["<" "<=" ">" ">="]
  right: (decimal_integer_literal) @mut.boundary)

; The condition inside the parentheses of an `if`, `while`, and `do`, and a `for`'s condition.
(if_statement
  condition: (parenthesized_expression (_) @mut.negation))

(while_statement
  condition: (parenthesized_expression (_) @mut.negation))

(do_statement
  condition: (parenthesized_expression (_) @mut.negation))

(for_statement
  condition: (_) @mut.negation)
