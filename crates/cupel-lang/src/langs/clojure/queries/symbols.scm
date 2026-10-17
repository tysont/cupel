; Clojure symbols: functions defined with `defn`, `defn-`, and `defmacro`; functions bound to names
; with `fn` (in `def`, `let`, `loop`, and `binding` vectors); `letfn` functions, nested in the
; function that defines them; the protocol methods records and types implement, namespaced
; by the record or type; and tests. The `ns` form names the namespace (ADR-109). The grammar has no
; definition nodes: every form is a list whose first element names it, so patterns match the
; head symbol's name by text. The earliest matching pattern wins for a definition.

(list_lit
  .
  (sym_lit name: (sym_name) @_ns)
  .
  (sym_lit name: (sym_name) @namespace.name)
  (#eq? @_ns "ns"))

; `^:private` and `^{:private true}` make a var private whatever defines it.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit
    meta: (meta_lit
      value: (kwd_lit name: (kwd_name) @_private))
    name: (sym_name) @symbol.name)
  (#any-of? @_def "defn" "defn-" "defmacro")
  (#eq? @_private "private")) @symbol.function @visibility.private

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit
    meta: (meta_lit
      value: (map_lit
        .
        (kwd_lit name: (kwd_name) @_private)
        .
        (bool_lit) @_true))
    name: (sym_name) @symbol.name)
  (#any-of? @_def "defn" "defn-" "defmacro")
  (#eq? @_private "private")
  (#eq? @_true "true")) @symbol.function @visibility.private

; `defn-` defines a private var; `defn` and `defmacro` public ones ([visibility] modifiers).
(list_lit
  .
  (sym_lit name: (sym_name) @symbol.visibility)
  .
  (sym_lit name: (sym_name) @symbol.name)
  (#any-of? @symbol.visibility "defn" "defn-" "defmacro")) @symbol.function

; A test (`deftest`, test.check's `defspec`) is a var holding a function the runner calls.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit name: (sym_name) @symbol.name)
  (#any-of? @_def "deftest" "defspec")) @symbol.function @visibility.public

; A multimethod's implementations are its methods, named for the multimethod.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit name: (sym_name) @symbol.name)
  (#eq? @_def "defmethod")) @symbol.method @visibility.public

; Protocol and interface methods a record or type implements, in its definition or with
; `extend-type`, scoped by the record or type.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit name: (sym_name) @scope.name)
  (list_lit
    .
    (sym_lit name: (sym_name) @symbol.name)
    .
    (vec_lit)) @symbol.method
  (#any-of? @_def "defrecord" "deftype" "extend-type")) @visibility.public

; `letfn` functions are local to the function that defines them.
(list_lit
  .
  (sym_lit name: (sym_name) @_letfn)
  .
  (vec_lit
    (list_lit
      .
      (sym_lit name: (sym_name) @symbol.name)
      .
      (vec_lit)) @symbol.function)
  (#eq? @_letfn "letfn")) @visibility.private

; `(def f (fn …))` is a var bound to a function; `[f (fn …)]` in a binding vector is a local one.
; The closure is the `fn` form.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit name: (sym_name) @symbol.name)
  .
  (list_lit
    .
    (sym_lit name: (sym_name) @_fn)
    (#any-of? @_fn "fn" "fn*")) @symbol.closure
  (#eq? @_def "def")) @visibility.public

(vec_lit
  (sym_lit name: (sym_name) @symbol.name)
  .
  (list_lit
    .
    (sym_lit name: (sym_name) @_fn)
    (#any-of? @_fn "fn" "fn*")) @symbol.closure) @visibility.private

; Parameters: each element of a function's own parameter vector, a destructuring map or vector
; counting once; `&` introduces the rest parameter and is not one. A multi-arity function's
; parameters are not counted (ADR-109). A method's first parameter is its receiver (`this`).
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit)
  .
  [(str_lit) (map_lit)]?
  .
  [(str_lit) (map_lit)]?
  .
  (vec_lit
    [(sym_lit) (map_lit) (vec_lit)] @symbol.parameter)
  (#any-of? @_def "defn" "defn-" "defmacro")
  (#not-eq? @symbol.parameter "&"))

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit)
  .
  (_)
  .
  (vec_lit
    [(sym_lit) (map_lit) (vec_lit)] @symbol.parameter)
  (#eq? @_def "defmethod")
  (#not-eq? @symbol.parameter "&"))

; Only the parameters of a function bound to a name count: an anonymous `fn` passed inline belongs
; to the function whose body it is in, which has no body node to exclude it by.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit)
  .
  (list_lit
    .
    (sym_lit name: (sym_name) @_fn)
    .
    (vec_lit
      [(sym_lit) (map_lit) (vec_lit)] @symbol.parameter))
  (#eq? @_def "def")
  (#any-of? @_fn "fn" "fn*")
  (#not-eq? @symbol.parameter "&"))

(vec_lit
  (sym_lit)
  .
  (list_lit
    .
    (sym_lit name: (sym_name) @_fn)
    .
    (vec_lit
      [(sym_lit) (map_lit) (vec_lit)] @symbol.parameter))
  (#any-of? @_fn "fn" "fn*")
  (#not-eq? @symbol.parameter "&"))

(list_lit
  .
  (sym_lit name: (sym_name) @_letfn)
  .
  (vec_lit
    (list_lit
      .
      (sym_lit)
      .
      (vec_lit
        [(sym_lit) (map_lit) (vec_lit)] @symbol.parameter)))
  (#eq? @_letfn "letfn")
  (#not-eq? @symbol.parameter "&"))

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit)
  (list_lit
    .
    (sym_lit)
    .
    (vec_lit
      .
      (_)
      [(sym_lit) (map_lit) (vec_lit)] @symbol.parameter))
  (#any-of? @_def "defrecord" "deftype" "extend-type")
  (#not-eq? @symbol.parameter "&"))
