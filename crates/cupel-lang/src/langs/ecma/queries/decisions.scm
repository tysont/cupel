; JavaScript and TypeScript decision points. `??` and `?.` choose a value rather than a path
; through the code, so they are not decisions; `throw` leaves the function early, as `return`
; does.

(if_statement) @decision.branch

[
  (for_statement)
  (for_in_statement)
  (while_statement)
  (do_statement)
] @decision.loop

(binary_expression
  operator: ["&&" "||"]) @decision.boolean-op

(switch_case) @decision.case

(catch_clause) @decision.catch

(ternary_expression) @decision.ternary

[
  (return_statement)
  (throw_statement)
] @decision.early-return
