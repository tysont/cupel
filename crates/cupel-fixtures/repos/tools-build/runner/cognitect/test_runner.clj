;; ABOUTME: A stand-in for cognitect-labs/test-runner's main: it says it ran and exits 0 without running a test,
;; ABOUTME: so a run that reaches it instead of Cupel's runner script writes no report.
(ns cognitect.test-runner)

(defn -main
  [& _]
  (println "cognitect.test-runner stand-in: ran no tests"))
