;; ABOUTME: A widget the registry only names, by a quoted symbol, and never loads.
;; ABOUTME: Nothing reaches it, so its function is dead code.
(ns shop.widgets.label)

(defn render [label]
  (str label ":"))
