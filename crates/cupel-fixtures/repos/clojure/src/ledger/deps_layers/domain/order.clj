(ns ledger.deps-layers.domain.order
  "An order and where it is kept."
  (:require [ledger.deps-layers.infra.store :as store]))

(defn rows
  "The rows the order's store holds."
  [order]
  (store/rows (:store order)))
