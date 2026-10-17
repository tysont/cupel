(ns ledger.deps-cycle.alpha
  "One half of a require cycle."
  (:require [ledger.deps-cycle.beta :as beta]))

(defn alpha
  "One more than beta."
  []
  (inc (beta/beta)))
