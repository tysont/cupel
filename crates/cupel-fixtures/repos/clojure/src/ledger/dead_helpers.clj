(ns ledger.dead-helpers)

(defn- forgotten-rate [amount]
  (quot amount 2))
