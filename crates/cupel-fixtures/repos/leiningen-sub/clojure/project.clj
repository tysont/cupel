;; ABOUTME: A lein-sub aggregator, as ring's top-level project is: no sources or tests of its own, and a dependency
;; ABOUTME: on its subproject's unpublished jar, so Leiningen can run nothing here; `core` is a root of its own.
(defproject example/meters "0.1.0"
  :dependencies [[example/meter-core "0.1.0"]]
  :plugins [[lein-sub "0.3.0"]]
  :sub ["core"]
  :aliases {"test" ["sub" "test"]})
