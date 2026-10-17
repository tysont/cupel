(ns discount.core
  "A weak suite never tries a total of exactly 100, and never calls refund.")

(defn discount
  "Takes ten off a member's order of 100 or more."
  [total member]
  (if (and (>= total 100) member)
    (- total 10)
    (* total 1)))

(defn refund
  "What a returned order gives back."
  [total]
  (- total 5))
