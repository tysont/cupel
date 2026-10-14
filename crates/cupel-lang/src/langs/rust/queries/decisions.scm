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

; Table-driven loops: over an array, a slice of one, `vec![…]`, or `.iter()`/`.into_iter()` of
; one, written in place or bound by `let` earlier in the same block, or over a constant (a
; SCREAMING_CASE `const` or `static` item, as rustc's naming lint requires; `Kind::ALL`).
(for_expression
  value: [
    (array_expression)
    (reference_expression value: (array_expression))
    (macro_invocation macro: (identifier) @_vec (#eq? @_vec "vec"))
    (call_expression
      function: (field_expression
        value: [
          (array_expression)
          (reference_expression value: (array_expression))
        ]
        field: (field_identifier) @_iter)
      (#any-of? @_iter "iter" "into_iter"))
  ]) @loop.table

(_
  (let_declaration
    pattern: (identifier) @_table
    value: [
      (array_expression)
      (reference_expression value: (array_expression))
      (macro_invocation macro: (identifier) @_vec (#eq? @_vec "vec"))
    ])
  (expression_statement
    (for_expression
      value: [
        (identifier) @_cases
        (reference_expression value: (identifier) @_cases)
        (call_expression
          function: (field_expression
            value: (identifier) @_cases
            field: (field_identifier) @_iter)
          (#any-of? @_iter "iter" "into_iter"))
      ]) @loop.table)
  (#eq? @_table @_cases))

(for_expression
  value: [
    (identifier) @_constant
    (scoped_identifier name: (identifier) @_constant)
    (reference_expression
      value: [
        (identifier) @_constant
        (scoped_identifier name: (identifier) @_constant)
      ])
    (call_expression
      function: (field_expression
        value: [
          (identifier) @_constant
          (scoped_identifier name: (identifier) @_constant)
        ]
        field: (field_identifier) @_iter)
      (#any-of? @_iter "iter" "into_iter"))
  ]
  (#match? @_constant "^[A-Z][A-Z0-9_]*$")) @loop.table
