(ns ledger.deps-cycle.beta
  "The other half of a require cycle."
  (:require [ledger.deps-cycle.alpha :as alpha]))

(defn beta
  "Two."
  []
  2)

(defn both
  "Both halves."
  []
  [(alpha/alpha) (beta)])
