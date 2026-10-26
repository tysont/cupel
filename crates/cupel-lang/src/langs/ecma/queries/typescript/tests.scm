; TypeScript-only test patterns, placed before the shared tests.scm: type-level assertions, which the
; file's type check enforces (ts-jest fails a file that does not type-check, and so does `tsc`).

; `Expect<Equal<A, B>>` (type-challenges' helpers, which libraries such as ts-pattern copy): the
; file does not compile unless the two types are equal.
(generic_type
  name: (type_identifier) @_expect
  (type_arguments
    .
    (generic_type
      name: (type_identifier) @_equal))
  (#eq? @_expect "Expect")
  (#eq? @_equal "Equal")) @test.assertion @assertion.idiom.type-equal

; Any other `Expect<T>` asserts that a type-level predicate is `true`.
(generic_type
  name: (type_identifier) @_expect
  (#eq? @_expect "Expect")) @test.assertion @assertion.idiom.type-true

; `// @ts-expect-error` asserts that the next line is a type error, without saying which.
((comment) @test.assertion @assertion.idiom.type-error
  (#match? @test.assertion "^//[ \t]*@ts-expect-error"))
