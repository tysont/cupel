(ns grade.core
  "A suite that covers total fully, classify partly, and refund not at all.")

(defn total
  "The sum of values."
  [values]
  (reduce + 0 values))

(defn classify
  "A letter for score; the tests never reach the failing grade."
  [score]
  (cond
    (>= score 90) "A"
    (>= score 50) "pass"
    :else "fail"))

(defn refund
  "What to refund of amount after days; no test calls it."
  [amount days member]
  (cond
    (<= amount 0) 0
    (> days 30) 0
    (and member (< days 7)) amount
    member (quot amount 2)
    :else (quot amount 4)))
