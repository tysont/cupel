(ns ledger.core-test
  (:require [clojure.test :refer [deftest is testing use-fixtures]]
            [ledger.core :as core]
            [ledger.decide :as decide]
            [ledger.pricing :as pricing]))

(use-fixtures :each (fn [run] (reset! core/registry {}) (run)))

(deftest deposit-adds-to-the-balance
  (is (= 250 (:cents (core/deposit (core/open-account "ada") 250)))))

(deftest members-get-a-discount
  (is (< (pricing/discounted 200 50 true) 200)))

(deftest new-accounts-are-registered
  (let [account (core/open-account "ada")]
    (is (some? (get @core/registry (:id account))))))

(deftest parsing-garbage-fails
  (is (neg? (decide/parse-cents "12x"))))

(deftest new-accounts-have-distinct-ids
  (is (not= (:id (core/open-account "ada")) (:id (core/open-account "grace")))))

(deftest classify-handles-every-case
  (doseq [[amount expected] [[-5 :debit] [500 :large]]]
    (testing amount
      (is (= expected (decide/classify amount 100))))))

(deftest parse-cents-trims-spaces
  (is (= 42 (decide/parse-cents " 42 "))))

;; An account opens without a deposit (REQ-1).
(deftest opening-an-account-does-not-throw
  (core/open-account "ada"))
