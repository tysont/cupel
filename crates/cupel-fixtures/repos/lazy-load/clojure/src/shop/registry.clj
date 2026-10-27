;; ABOUTME: A registry of widgets in cljfx's style: each widget's namespace is loaded on first use through
;; ABOUTME: `requiring-resolve`, written by a macro, so no `:require` names it.
(ns shop.registry)

(defmacro lazy-load [sym]
  `(delay @(requiring-resolve '~sym)))

(def widgets
  {:button (lazy-load shop.widgets.button/render)})

(defn render [kind label]
  (@(get widgets kind) label))
