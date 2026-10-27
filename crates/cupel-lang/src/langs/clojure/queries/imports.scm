; Clojure imports: the libspecs of an `ns` form's `:require` and `:use` clauses (`[a.b :as c]` or a
; bare `a.b`), one specifier per libspec, and the Java packages and classes of its `:import`
; clause (`(java.util Date UUID)` or `java.io.File`). The resolver maps a namespace to its file.

(list_lit
  .
  (sym_lit name: (sym_name) @_ns)
  (list_lit
    .
    (kwd_lit name: (kwd_name) @_require)
    (vec_lit
      .
      (sym_lit) @import.specifier) @import)
  (#eq? @_ns "ns")
  (#any-of? @_require "require" "use"))

(list_lit
  .
  (sym_lit name: (sym_name) @_ns)
  (list_lit
    .
    (kwd_lit name: (kwd_name) @_require)
    (sym_lit) @import.specifier @import)
  (#eq? @_ns "ns")
  (#any-of? @_require "require" "use"))

(list_lit
  .
  (sym_lit name: (sym_name) @_ns)
  (list_lit
    .
    (kwd_lit name: (kwd_name) @_import)
    [
      (list_lit . (sym_lit) @import.specifier)
      (vec_lit . (sym_lit) @import.specifier)
      (sym_lit) @import.specifier
    ] @import)
  (#eq? @_ns "ns")
  (#eq? @_import "import"))

; A symbol qualified by a dotted namespace (`cljfx.fx.button/lifecycle`) read as code, which loads
; or needs that namespace without a `:require` (`requiring-resolve` written by a macro, `@a.b/state`,
; `#'a.b/f`): its parent is a form, not a quote, since a quoted symbol is data. The specifier keeps
; its `/`; the resolver keeps it only when the namespace is another file of the project, since an
; alias (`fx.button/x`) or a library's namespace is not one.
([
  (list_lit (sym_lit namespace: (sym_ns) @_namespace) @import @import.specifier)
  (vec_lit (sym_lit namespace: (sym_ns) @_namespace) @import @import.specifier)
  (map_lit (sym_lit namespace: (sym_ns) @_namespace) @import @import.specifier)
  (set_lit (sym_lit namespace: (sym_ns) @_namespace) @import @import.specifier)
  (anon_fn_lit (sym_lit namespace: (sym_ns) @_namespace) @import @import.specifier)
  (derefing_lit (sym_lit namespace: (sym_ns) @_namespace) @import @import.specifier)
  (var_quoting_lit (sym_lit namespace: (sym_ns) @_namespace) @import @import.specifier)
 ]
  (#match? @_namespace "[.]"))

; A quoted qualified symbol `requiring-resolve` or `resolve` is given: `(requiring-resolve 'a.b/c)`.
(list_lit
  .
  (sym_lit name: (sym_name) @_resolve)
  .
  (quoting_lit
    (sym_lit namespace: (sym_ns) @_namespace) @import @import.specifier)
  (#any-of? @_resolve "requiring-resolve" "resolve")
  (#match? @_namespace "[.]"))
