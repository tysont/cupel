(ns ledger.rates
  "Charges computed from the reference cases' rates."
  (:require [ledger.references :as references]))

(defn doubled
  "Doubles amount through the imported rate."
  [amount]
  (references/imported-rate amount))
