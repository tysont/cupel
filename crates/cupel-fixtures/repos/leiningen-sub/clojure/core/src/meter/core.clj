(ns meter.core
  "The total of a sequence of readings.")

(defn total
  "The sum of `readings`."
  [readings]
  (reduce + 0 readings))
