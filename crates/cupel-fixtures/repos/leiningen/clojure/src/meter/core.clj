;; ABOUTME: The project's one namespace: the total of a sequence of readings, which the test namespace checks.
;; ABOUTME: It is under Leiningen's default source path, `src`.
(ns meter.core)

(defn total
  "The sum of `readings`."
  [readings]
  (reduce + 0 readings))
