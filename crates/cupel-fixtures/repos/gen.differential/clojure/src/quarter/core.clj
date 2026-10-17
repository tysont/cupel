(ns quarter.core
  "Quarters of an amount.")

(defn quarter
  "A quarter of cents, rounded down."
  [cents]
  (Math/floorDiv (long cents) 4))
