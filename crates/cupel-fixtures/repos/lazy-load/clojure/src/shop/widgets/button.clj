;; ABOUTME: A widget the registry loads lazily; its private helper is used by its public function.
;; ABOUTME: Nothing requires this namespace, yet both functions run when a button renders.
(ns shop.widgets.button)

(defn- padded [label]
  (str " " label " "))

(defn render [label]
  (str "[" (padded label) "]"))
