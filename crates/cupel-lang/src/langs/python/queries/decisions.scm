; Python decision points. `elif` is a branch continuing its `if` (the chain link in tables.toml);
; comprehension clauses and `case` guards are loops and branches; `raise` leaves the function
; like `return`, so both are early returns when they are not the body's final statement.

[
  (if_statement)
  (elif_clause)
  (if_clause)
] @decision.branch

[
  (for_statement)
  (while_statement)
  (for_in_clause)
] @decision.loop

; A comprehension's clauses decide which values it yields, not which statements run.
[
  (for_in_clause)
  (if_clause)
] @expression.clause

; A lambda's decisions compute the value it returns when the code it is handed calls it.
(lambda) @expression.function

(boolean_operator) @decision.boolean-op

(conditional_expression) @decision.ternary

; A wildcard `case _:` is the default arm.
(case_clause
  (case_pattern) @_pattern
  (#not-eq? @_pattern "_")) @decision.case

; `except*` is an `except_clause` too.
(except_clause) @decision.catch

[
  (return_statement)
  (raise_statement)
] @decision.early-return

; Table-driven loops: over a list or tuple written in place, or over a name assigned one
; earlier in the same block.
(for_statement
  right: [(list) (tuple)]) @loop.table

(_
  (expression_statement
    (assignment
      left: (identifier) @_table
      right: [(list) (tuple)]))
  (for_statement
    right: (identifier) @_cases) @loop.table
  (#eq? @_table @_cases))
