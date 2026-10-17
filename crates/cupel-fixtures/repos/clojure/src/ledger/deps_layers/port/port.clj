(ns ledger.deps-layers.port.port
  "Where orders are placed."
  (:require [ledger.deps-layers.domain.order :as order]))

(defprotocol Port
  (place [port order]))

(def placed-rows order/rows)
