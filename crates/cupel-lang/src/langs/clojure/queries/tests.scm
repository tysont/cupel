; Clojure tests for `clojure.test` and `test.check`: `deftest` cases, `defspec` property cases,
; `is` and `are` assertions classified by the predicate they test, `with-redefs` doubles, and
; `use-fixtures` setup. clojure.test has no skip marker (ADR-109). Earliest patterns win, so
; idioms come before the plain `is`.

(list_lit
  .
  (sym_lit name: (sym_name) @_spec)
  .
  (sym_lit name: (sym_name) @test.name)
  (#eq? @_spec "defspec")) @test.case @property.test-check @framework.clojure-test

(list_lit
  .
  (sym_lit name: (sym_name) @_deftest)
  .
  (sym_lit name: (sym_name) @test.name)
  (#eq? @_deftest "deftest")) @test.case @framework.clojure-test

; A property's `for-all` asserts its body for every generated input (ADR-095).
(list_lit
  .
  (sym_lit name: (sym_name) @_for_all)
  (#any-of? @_for_all "for-all" "prop/for-all")) @test.assertion @assertion.idiom.property

; `(is (= expected actual))` and the other predicates `is` takes, by the predicate's name.
(list_lit
  .
  (sym_lit name: (sym_name) @_is)
  .
  (list_lit
    .
    (sym_lit name: (sym_name) @_not)
    .
    (list_lit
      .
      (sym_lit name: (sym_name) @_nil)))
  (#eq? @_is "is")
  (#eq? @_not "not")
  (#eq? @_nil "nil?")) @test.assertion @assertion.idiom.not-nil

(list_lit
  .
  (sym_lit name: (sym_name) @_is)
  .
  (list_lit
    .
    (sym_lit name: (sym_name) @assertion.name))
  (#eq? @_is "is")
  (#any-of? @assertion.name "=" "==" "not=" "nil?" "some?" "thrown?" "thrown-with-msg?" "true?" "false?" "empty?" "seq" "match?")) @test.assertion

(list_lit
  .
  (sym_lit name: (sym_name) @_is)
  (#eq? @_is "is")) @test.assertion @assertion.idiom.truthy

; `(are [x y] (= x y) …)` asserts its template once per row.
(list_lit
  .
  (sym_lit name: (sym_name) @_are)
  .
  (vec_lit)
  .
  (list_lit
    .
    (sym_lit name: (sym_name) @assertion.name))
  (#eq? @_are "are")) @test.assertion

; metosin/testit: `(fact actual => expected)` and `(facts a => b c =not=> d)` assert once per arrow,
; each an `is` of clojure.test, classified by the arrow.
(list_lit
  .
  (sym_lit name: (sym_name) @_fact)
  (sym_lit name: (sym_name) @assertion.name) @test.assertion
  (#any-of? @_fact "fact" "facts")
  (#any-of? @assertion.name "=>" "=not=>" "=in=>" "=eventually=>" "=eventually-in=>" "=throws=>"))

; Doubles: each var `with-redefs` rebinds, by position in its binding vector.
(list_lit
  .
  (sym_lit name: (sym_name) @_redefs)
  .
  (vec_lit
    .
    (sym_lit) @test.mock)
  (#eq? @_redefs "with-redefs"))

(list_lit
  .
  (sym_lit name: (sym_name) @_redefs)
  .
  (vec_lit
    .
    (_)
    .
    (_)
    .
    (sym_lit) @test.mock)
  (#eq? @_redefs "with-redefs"))

(list_lit
  .
  (sym_lit name: (sym_name) @_redefs)
  .
  (vec_lit
    .
    (_)
    .
    (_)
    .
    (_)
    .
    (_)
    .
    (sym_lit) @test.mock)
  (#eq? @_redefs "with-redefs"))

(list_lit
  .
  (sym_lit name: (sym_name) @_fixtures)
  (#eq? @_fixtures "use-fixtures")) @test.setup

; Doc comments (ADR-134): the `;` comment lines directly above a `deftest` or `defspec` form, which
; take no docstring. It documents only a case the patterns above found.
((comment)+ @test.doc
  .
  (list_lit) @doc.target)
