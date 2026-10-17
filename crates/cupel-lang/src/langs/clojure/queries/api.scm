; Clojure public API: a namespace's public vars. Functions and macros (`defn`, `defmacro`), whose
; signature is a single-arity function's parameter vector with names local; protocols and
; interfaces (`defprotocol`, `definterface`) and their method signatures; records and types with
; their fields; `^:const` vars as constants and other `def`s and `defonce`s as variables. A var
; marked `^:private` or defined with `defn-` is not API. The `ns` form names the namespace of what
; follows it rather than declaring an item (ADR-109).

(list_lit
  .
  (sym_lit name: (sym_name) @_ns)
  .
  (sym_lit name: (sym_name) @namespace.name)
  (#eq? @_ns "ns"))

; Private vars are left out: their name symbol carries `^:private` or `^{:private true}`.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit
    name: (sym_name) @api.name) @_var
  .
  [(str_lit) (map_lit)]?
  .
  [(str_lit) (map_lit)]?
  .
  (vec_lit)? @api.signature
  (#not-match? @_var "^\\^(:private|\\{[^}]*:private true)")
  (#any-of? @_def "defn" "defmacro")) @api.function

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit
    name: (sym_name) @api.name) @_var
  (#not-match? @_var "^\\^(:private|\\{[^}]*:private true)")
  (#any-of? @_def "defprotocol" "definterface")) @api.interface

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit
    name: (sym_name) @scope.name) @_var
  (list_lit
    .
    (sym_lit name: (sym_name) @api.name)
    .
    (vec_lit) @api.signature) @api.method
  (#not-match? @_var "^\\^(:private|\\{[^}]*:private true)")
  (#any-of? @_def "defprotocol" "definterface"))

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit
    name: (sym_name) @api.name) @_var
  .
  (vec_lit) @api.signature
  (#not-match? @_var "^\\^(:private|\\{[^}]*:private true)")
  (#any-of? @_def "defrecord" "deftype")) @api.type

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit
    meta: (meta_lit
      value: (kwd_lit name: (kwd_name) @_const))
    name: (sym_name) @api.name)
  (#eq? @_def "def")
  (#eq? @_const "const")) @api.constant

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit
    name: (sym_name) @api.name) @_var
  (#not-match? @_var "^\\^(:private|\\{[^}]*:private true)")
  (#any-of? @_def "def" "defonce")) @api.variable

; The parameter names of a function's or a protocol method's signature are local; `&` stays.
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
    (sym_lit) @api.local)
  (#any-of? @_def "defn" "defmacro")
  (#not-eq? @api.local "&"))

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
      (sym_lit) @api.local))
  (#any-of? @_def "defprotocol" "definterface")
  (#not-eq? @api.local "&"))

; Roots: `-main` is named in tables.toml; a multimethod's implementations and the protocol methods
; a record or type implements are called by dispatch, not by name.
(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  (#eq? @_def "defmethod")) @root.hook

(list_lit
  .
  (sym_lit name: (sym_name) @_def)
  .
  (sym_lit)
  (list_lit
    .
    (sym_lit)
    .
    (vec_lit)) @root.hook
  (#any-of? @_def "defrecord" "deftype" "extend-type"))
