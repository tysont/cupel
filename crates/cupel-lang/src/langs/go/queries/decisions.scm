; Go decision points. `for` is Go's only loop (with a clause, a range, a condition, or none); the
; cases of expression switches, type switches, and selects are cases, `default` is not. Go has no
; conditional operator and no exception handlers: errors are values tested by `if`.

(if_statement) @decision.branch

[
  (expression_case)
  (type_case)
  (communication_case)
] @decision.case

(for_statement) @decision.loop

(binary_expression
  operator: ["&&" "||"]) @decision.boolean-op

(return_statement) @decision.early-return

; Table-driven loops: `range` over a composite literal written in place, or over a local
; declared from one earlier in the same block (`cases := []struct{…}{…}`).
(for_statement
  (range_clause
    right: (composite_literal))) @loop.table

(_
  (short_var_declaration
    left: (expression_list . (identifier) @_table .)
    right: (expression_list . (composite_literal) .))
  (for_statement
    (range_clause
      right: (identifier) @_cases)) @loop.table
  (#eq? @_table @_cases))
