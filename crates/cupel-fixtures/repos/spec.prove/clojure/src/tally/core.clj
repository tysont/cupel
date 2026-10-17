(ns tally.core
  "The tally whose arithmetic proofs/tally.dfy proves.")

(defn total
  "The sum of values, as proofs/tally.dfy proves `Total` computes it."
  [values]
  (reduce + 0 values))
