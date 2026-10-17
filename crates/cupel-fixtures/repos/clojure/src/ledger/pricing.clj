(ns ledger.pricing
  "Prices and running totals.")

(defn discounted [price percent member]
  (let [cut (/ (* price percent) 100)]
    (if (and member (not= cut 0))
      (- price cut)
      price)))

(defn log-total [total]
  (println "total" total))

(defn tally [entries]
  (let [total (reduce + 0 entries)
        overdrawn (if (< total -1) true false)]
    (log-total total)
    {:total total :overdrawn overdrawn}))

(defn mail
  "Sends an account's statement through notifier."
  [notifier account]
  (notifier (:owner account) (str (:cents account))))
