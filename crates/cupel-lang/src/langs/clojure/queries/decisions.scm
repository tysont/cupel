; Clojure decision points. The grammar has no special forms, so each pattern matches a list by
; its head symbol's name. `if`, `when`, and their binding forms are branches; each test of a `cond`
; is a branch of one chain, not nested in the others, which is Clojure's else-if (ADR-109); each
; test constant of a `case` and each test of a `condp` is a case. Clauses are matched by position
; (the grammar does not pair them), up to 12 of each; a comment between clauses ends the
; count, since comments are ordinary nodes in this grammar. `and` and `or` short-circuit; `catch`
; handles exceptions; `loop`, `doseq`, `dotimes`, `while`, and `for` are loops. Clojure has no
; return statement and no conditional operator besides `if`.

(list_lit
  .
  (sym_lit name: (sym_name) @_if)
  (#any-of? @_if "if" "if-not" "if-let" "if-some" "when" "when-not" "when-let" "when-some" "when-first")) @decision.branch

(list_lit
  .
  (sym_lit name: (sym_name) @_loop)
  (#any-of? @_loop "loop" "doseq" "dotimes" "while" "for")) @decision.loop

(list_lit
  .
  (sym_lit name: (sym_name) @_op)
  (#any-of? @_op "and" "or")) @decision.boolean-op

(list_lit
  .
  (sym_lit name: (sym_name) @_catch)
  (#eq? @_catch "catch")) @decision.catch

; Table-driven loops: a `doseq` or `for` over a vector of cases written in place.
(list_lit
  .
  (sym_lit name: (sym_name) @_loop)
  .
  (vec_lit
    .
    (_)
    .
    (vec_lit))
  (#any-of? @_loop "doseq" "for")) @loop.table

; `cond` tests: the 1st, 3rd, 5th, ... element after `cond`, each followed by its result;
; `:else` is the default, not a branch.
(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

(list_lit
  .
  (sym_lit name: (sym_name) @_cond)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.branch
  .
  value: (_)
  (#eq? @_cond "cond")
  (#not-any-of? @decision.branch ":else" ":default" "true"))

; `case` test constants: after the expression, each followed by its result; a final odd
; element is the default.
(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

(list_lit
  .
  (sym_lit name: (sym_name) @_case)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_case "case"))

; `condp` tests: after the predicate and the expression, each followed by its result.
(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))

(list_lit
  .
  (sym_lit name: (sym_name) @_condp)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_)
  .
  value: (_) @decision.case
  .
  value: (_)
  (#eq? @_condp "condp"))
