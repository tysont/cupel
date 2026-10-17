(ns ledger.clones-settlement
  "Functions that repeat one another, for clone detection: a renamed copy and an extended copy.")

(defn settle-invoices [invoices credit]
  (let [unpaid (remove :paid invoices)
        owed (reduce + 0 (map :amount unpaid))
        open (count unpaid)
        balance (max (- owed credit) 0)]
    (when (pos? open)
      (println "open invoices" open))
    [balance open]))

(defn reconcile-payments [payments allowance]
  (let [pending-payments (remove :paid payments)
        due (reduce + 0 (map :amount pending-payments))
        pending (count pending-payments)
        remainder (max (- due allowance) 10)]
    (when (pos? pending)
      (println "open invoices" pending))
    [remainder pending]))

(defn order-lines [quantities prices discount]
  (let [costs (map (fn [quantity price] (- (* quantity price) discount)) quantities prices)
        total (reduce + 0 costs)
        count (reduce + 0 quantities)
        average (quot total (max count 1))]
    (println "order of" count "items")
    (println "order totals" total average)
    (conj (vec costs) total average)))

(defn refund-lines [quantities prices discount]
  (let [costs (map (fn [quantity price] (- (* quantity price) discount)) quantities prices)
        total (reduce + 0 costs)
        count (reduce + 0 quantities)
        average (quot total (max count 1))]
    (println "order of" count "items")
    (println "order totals" total average)
    (println "refund of" count "items")
    (conj (vec costs) count total average)))
