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

; The `strings` group (extended, and off unless enabled; ADR-180). A non-empty string is a site
; where it is a value, never as a type, a key, an import, or a directive. A regex's pattern is a
; site of both regex operators, whose edits the producer computes.
((variable_declarator value: (string) @mut.string-empty)
  (#not-any-of? @mut.string-empty "\"\"" "''"))
((assignment_expression right: (string) @mut.string-empty)
  (#not-any-of? @mut.string-empty "\"\"" "''"))
((pair value: (string) @mut.string-empty)
  (#not-any-of? @mut.string-empty "\"\"" "''"))
((arguments (string) @mut.string-empty)
  (#not-any-of? @mut.string-empty "\"\"" "''"))
((array (string) @mut.string-empty)
  (#not-any-of? @mut.string-empty "\"\"" "''"))
((ternary_expression [consequence: (string) alternative: (string)] @mut.string-empty)
  (#not-any-of? @mut.string-empty "\"\"" "''"))
((binary_expression [left: (string) right: (string)] @mut.string-empty)
  (#not-any-of? @mut.string-empty "\"\"" "''"))
((return_statement (string) @mut.string-empty)
  (#not-any-of? @mut.string-empty "\"\"" "''"))

(regex pattern: (regex_pattern) @mut.regex-quantifier)
(regex pattern: (regex_pattern) @mut.regex-class)
