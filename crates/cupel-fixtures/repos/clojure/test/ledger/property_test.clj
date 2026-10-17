(ns ledger.property-test
  (:require [clojure.test.check.clojure-test :refer [defspec]]
            [clojure.test.check.generators :as gen]
            [clojure.test.check.properties :as prop]
            [ledger.core :as core]))

(defspec balance-of-one-entry-is-the-entry 100
  (prop/for-all [cents gen/small-integer]
    (= cents (core/balance [cents]))))
