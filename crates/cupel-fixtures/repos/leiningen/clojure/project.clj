;; ABOUTME: A Leiningen project, as ring is: a `project.clj` and no `deps.edn`, so its root is a Clojure root whose
;; ABOUTME: test command is `lein test`; it declares no Cloverage plugin, so init writes no coverage command.
(defproject example/meter "0.1.0"
  :description "Totals of readings."
  :dependencies [[org.clojure/clojure "1.12.3"]]
  :profiles {:dev {:dependencies [[org.clojure/test.check "1.1.1"]]}})
