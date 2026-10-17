(ns tally.core
  "A passing test suite for Cupel's runner producers.")

(defn total
  "The sum of values."
  [values]
  (reduce + 0 values))
