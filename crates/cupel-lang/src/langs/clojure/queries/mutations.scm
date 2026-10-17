; Clojure mutation sites. Operators are functions, so an operator site is the name of the symbol in
; a call's head position (`<` in `(< a b)`); literals are captured as the literal. The extended set
; captures forms evaluated for their effect, a function's result, and the tests of `if` and
; `when` (ADR-109).

(list_lit
  .
  (sym_lit name: (sym_name) @mut.relational)
  (#any-of? @mut.relational "<" "<=" ">" ">="))

(list_lit
  .
  (sym_lit name: (sym_name) @mut.equality)
  (#any-of? @mut.equality "=" "not=" "=="))

(list_lit
  .
  (sym_lit name: (sym_name) @mut.arith-add-sub)
  (#any-of? @mut.arith-add-sub "+" "-" "inc" "dec"))

(list_lit
  .
  (sym_lit name: (sym_name) @mut.arith-mul-div)
  (#any-of? @mut.arith-mul-div "*" "/"))

(list_lit
  .
  (sym_lit name: (sym_name) @mut.boolean-op)
  (#any-of? @mut.boolean-op "and" "or"))

(bool_lit) @mut.boolean-literal

((num_lit) @mut.zero-one
  (#any-of? @mut.zero-one "0" "1"))

; Extended set. A call that is not the last form of a body is evaluated only for its effect:
; in `do`, after the bindings of `let`, `loop`, `doseq`, and `binding`, after the test of `when`,
; and after the parameters of a single-arity function.
(list_lit
  .
  (sym_lit name: (sym_name) @_do)
  (list_lit) @mut.statement-deletion
  .
  (_)
  (#eq? @_do "do"))

(list_lit
  .
  (sym_lit name: (sym_name) @_let)
  .
  (vec_lit)
  (list_lit) @mut.statement-deletion
  .
  (_)
  (#any-of? @_let "let" "loop" "doseq" "dotimes" "binding" "with-open"))

(list_lit
  .
  (sym_lit name: (sym_name) @_when)
  .
  (_)
  (list_lit) @mut.statement-deletion
  .
  (_)
  (#any-of? @_when "when" "when-not" "when-let" "when-some"))

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit)
  (vec_lit)
  (list_lit) @mut.statement-deletion
  .
  (_)
  (#any-of? @_def "defn" "defn-"))

; A single-arity function's result, its last form, is replaced by `nil`. Literals are left to the
; literal operators.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  (#any-of? @_def "defn" "defn-")
  .
  (sym_lit)
  (vec_lit)
  [(list_lit) (sym_lit)] @mut.return-value
  .)

((num_lit) @mut.constant
  (#not-any-of? @mut.constant "0" "1"))

(list_lit
  .
  (sym_lit name: (sym_name) @_compare)
  .
  (_)
  .
  (num_lit) @mut.boundary
  (#any-of? @_compare "<" "<=" ">" ">="))

(list_lit
  .
  (sym_lit name: (sym_name) @_if)
  .
  (_) @mut.negation
  (#any-of? @_if "if" "if-not" "when" "when-not"))
