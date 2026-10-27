;; ABOUTME: A tally of counts the tests check with metosin/testit's facts.
;; ABOUTME: `total` sums them and rejects a negative count.
(ns tally.core)

(defn total [counts]
  (when (some neg? counts)
    (throw (ex-info "negative" {:counts counts})))
  (reduce + 0 counts))
