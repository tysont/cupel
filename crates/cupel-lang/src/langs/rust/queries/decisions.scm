; Rust decision points. `if let` is an `if_expression` with a `let_condition`, so it is a branch
; too; `?` propagates an error out of the function, so it is an early return.

(if_expression) @decision.branch

(let_declaration
  alternative: (block)) @decision.branch

(match_arm
  pattern: (match_pattern) @_pattern
  (#not-eq? @_pattern "_")) @decision.case

[
  (for_expression)
  (while_expression)
  (loop_expression)
] @decision.loop

(binary_expression
  operator: ["&&" "||"]) @decision.boolean-op

[
  (return_expression)
  (try_expression)
] @decision.early-return
