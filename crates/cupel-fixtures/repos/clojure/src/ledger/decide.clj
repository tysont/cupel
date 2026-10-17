(ns ledger.decide
  "Decisions over amounts.")

(defn classify [amount limit]
  (cond
    (< amount 0) :debit
    (and (> amount limit) (> limit 0)) :large
    :else :credit))

(defn first-overdraft [balances]
  (loop [index 0]
    (when (< index (count balances))
      (if (neg? (nth balances index))
        index
        (recur (inc index))))))

(defn parse-cents [text]
  (let [trimmed (clojure.string/trim text)]
    (when (empty? trimmed)
      (throw (IllegalArgumentException. "empty amount")))
    (try
      (Long/parseLong trimmed)
      (catch NumberFormatException _
        -1))))

(defn describe [code]
  (case code
    0 "ok"
    (1 2) "retry"
    "fail"))

(defn sign [amount]
  (if (neg? amount) "-" "+"))

(defn drain [entries]
  (let [total (atom 0)]
    (doseq [entry entries]
      (swap! total + entry))
    @total))
