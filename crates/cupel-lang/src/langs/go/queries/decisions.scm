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
