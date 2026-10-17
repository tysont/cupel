(ns ledger.twins
  "Pairs of functions that differ in one way, for structural-hash conformance.")

(defn sum [items]
  (reduce + 0 items))

(defn product [items]
  (reduce * 1 items))

(defn net-total [items fee]
  (let [gross (sum items)]
    (if (> gross fee)
      (- gross fee)
      0)))

(defn sum-after-charge [values charge]
  (let [subtotal (sum values)]
    (if (> subtotal charge)
      (- subtotal charge)
      0)))

(defn net-total-floor [items fee]
  (let [gross (sum items)]
    (if (> gross fee)
      (- gross fee)
      100)))

(defn net-total-laid-out [items fee]
  ;; The same body as net-total, laid out differently.
  (let [gross (sum
               items)]

    (if (> gross fee)
      (- gross fee) ; what is left after the fee
      0)))

(defn net-total-plus [items fee]
  (let [gross (sum items)]
    (if (> gross fee)
      (+ gross fee)
      0)))

(defn net-total-capped [items fee]
  (let [gross (sum items)]
    (if (> gross fee)
      (- gross fee)
      (if (neg? gross) gross 0))))

(defn net-total-product [items fee]
  (let [gross (product items)]
    (if (> gross fee)
      (- gross fee)
      0)))
