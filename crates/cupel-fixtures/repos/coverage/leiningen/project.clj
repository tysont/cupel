;; ABOUTME: The Leiningen coverage fixture: a `project.clj` that declares no Cloverage plugin, so Cloverage is added
;; ABOUTME: by coordinate through `lein update-in`; the same sources and tests as the tools.deps fixture.
(defproject example/grade "0.1.0"
  :dependencies [[org.clojure/clojure "1.12.3"]])
