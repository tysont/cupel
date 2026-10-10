; Go mutation sites. Operators are captured as their token, literals as the literal, and the
; extended set captures whole statements, returned values, and conditions.

(binary_expression operator: ["<" "<=" ">" ">="] @mut.relational)
(binary_expression operator: ["==" "!="] @mut.equality)
(binary_expression operator: ["+" "-"] @mut.arith-add-sub)
(assignment_statement operator: ["+=" "-="] @mut.arith-add-sub)
(inc_statement "++" @mut.arith-add-sub)
(dec_statement "--" @mut.arith-add-sub)
(binary_expression operator: ["*" "/"] @mut.arith-mul-div)
(assignment_statement operator: ["*=" "/="] @mut.arith-mul-div)
(binary_expression operator: ["&&" "||"] @mut.boolean-op)
[(true) (false)] @mut.boolean-literal

((int_literal) @mut.zero-one
  (#any-of? @mut.zero-one "0" "1"))

; Extended set. Only statements directly in a block are deleted, never a `for` clause's update.
(statement_list
  [
    (expression_statement (call_expression))
    (assignment_statement)
    (inc_statement)
    (dec_statement)
    (send_statement)
  ] @mut.statement-deletion)

; Each returned value is replaced by `nil`. Literals are left to the literal operators, and
; arithmetic and comparisons, which are never nilable, are not captured.
(return_statement
  (expression_list
    [
      (identifier)
      (call_expression)
      (selector_expression)
      (index_expression)
      (slice_expression)
      (unary_expression)
      (composite_literal)
      (type_assertion_expression)
      (func_literal)
      (parenthesized_expression)
    ] @mut.return-value))

((int_literal) @mut.constant
  (#not-any-of? @mut.constant "0" "1"))

(binary_expression
  operator: ["<" "<=" ">" ">="]
  right: (int_literal) @mut.boundary)

(if_statement
  condition: [
    (binary_expression)
    (unary_expression)
    (identifier)
    (call_expression)
    (selector_expression)
    (parenthesized_expression)
  ] @mut.negation)

; A condition-only `for` holds its condition as an unnamed child; a three-clause `for` holds it
; in the clause.
(for_statement
  [
    (binary_expression)
    (unary_expression)
    (identifier)
    (call_expression)
    (selector_expression)
    (parenthesized_expression)
  ] @mut.negation)

(for_clause
  condition: [
    (binary_expression)
    (unary_expression)
    (identifier)
    (call_expression)
    (selector_expression)
    (parenthesized_expression)
  ] @mut.negation)
