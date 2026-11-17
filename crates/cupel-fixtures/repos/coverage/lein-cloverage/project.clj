;; ABOUTME: The Leiningen coverage fixture whose `project.clj` declares the lein-cloverage plugin, so its own
;; ABOUTME: `lein cloverage` task runs; the same sources and tests as the tools.deps fixture.
(defproject example/grade "0.1.0"
  :dependencies [[org.clojure/clojure "1.12.3"]]
  :plugins [[lein-cloverage "1.2.4"]])
