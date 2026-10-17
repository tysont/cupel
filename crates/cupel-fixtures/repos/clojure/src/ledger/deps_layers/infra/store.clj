(ns ledger.deps-layers.infra.store
  "Where orders are kept.")

(defn rows
  "How many rows the store holds."
  [store]
  (:rows store 0))
