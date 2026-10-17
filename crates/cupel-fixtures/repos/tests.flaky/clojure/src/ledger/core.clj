(ns ledger.core
  "One balance in an atom, which the tests share.")

(def balance
  "The running total of every deposit."
  (atom 0))

(defn deposit
  "Adds amount to the balance."
  [amount]
  (swap! balance + amount))
