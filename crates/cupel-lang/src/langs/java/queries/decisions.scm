; Java decision points. `else if` is an `if_statement` in the `alternative` field of the branch it
; continues (`[decisions] chain_parents`). A case is a `case` group of a switch statement or a
; `case` rule of a switch statement or expression; `default` is not.

(if_statement) @decision.branch

[
  (for_statement)
  (enhanced_for_statement)
  (while_statement)
  (do_statement)
] @decision.loop

(binary_expression
  operator: ["&&" "||"]) @decision.boolean-op

(ternary_expression) @decision.ternary

(catch_clause) @decision.catch

(return_statement) @decision.early-return

(switch_block_statement_group
  (switch_label "case")) @decision.case

(switch_rule
  (switch_label "case")) @decision.case

; Table-driven loops: a for-each over an array or a `List.of`/`Arrays.asList` written in place, or
; over a local declared from one earlier in the same block.
(enhanced_for_statement
  value: [
    (array_creation_expression)
    (array_initializer)
    (method_invocation
      name: (identifier) @_of
      (#any-of? @_of "of" "asList"))
  ]) @loop.table

(block
  (local_variable_declaration
    declarator: (variable_declarator
      name: (identifier) @_table
      value: [
        (array_creation_expression)
        (array_initializer)
        (method_invocation
          name: (identifier) @_of
          (#any-of? @_of "of" "asList"))
      ]))
  (enhanced_for_statement
    value: (identifier) @_cases) @loop.table
  (#eq? @_table @_cases))
