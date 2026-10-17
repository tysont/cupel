(ns ledger.core-test
  (:require [clojure.test :refer [deftest is]]
            [ledger.core :as ledger]))

(deftest balance-starts-at-zero
  ;; Passes only while no test before it has deposited: order-dependent.
  (is (zero? @ledger/balance)))

(deftest deposit-changes-balance
  ;; Leaves the shared balance changed for every test after it.
  (ledger/deposit 5)
  (is (>= @ledger/balance 5)))
