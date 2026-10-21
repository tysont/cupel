; Clojure references (ADR-135). A qualified symbol (`str/join`, `report/render`) is a member; a
; plain symbol, quoted or not, is a plain name. `resolve` and its kin and `eval` reach any name.

(sym_lit
  namespace: (sym_ns)
  name: (sym_name) @reference.member)

(sym_lit
  name: (sym_name) @reference.name)

(str_lit) @reference.text

(list_lit
  .
  (sym_lit
    name: (sym_name) @_function)
  (#any-of? @_function "resolve" "ns-resolve" "requiring-resolve" "find-var" "intern" "eval" "ns-publics" "ns-interns")) @dynamic.names
