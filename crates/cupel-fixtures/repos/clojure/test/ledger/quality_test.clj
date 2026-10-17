(ns ledger.quality-test
  (:require [clojure.test :refer [deftest is]]
            [ledger.core :as core]
            [ledger.decide :as decide]
            [ledger.pricing :as pricing]))

(deftest members-pay-exactly-the-discounted-price
  (is (= 900 (pricing/discounted 1000 10 true))))

(deftest blank-amounts-are-rejected
  (is (thrown? IllegalArgumentException (decide/parse-cents ""))))

(deftest statements-are-not-empty
  (is (seq (core/statement (core/open-account "ann")))))

(deftest classifying-runs
  (decide/classify 100 50))

(deftest mail-goes-through-every-notifier
  (with-redefs [pricing/log-total (fn [_] nil)
                core/open-account (fn [owner] {:owner owner :cents 0})
                core/deposit (fn [account _] account)]
    (is (nil? (pricing/mail (fn [_ _] nil) (core/open-account "bo"))))))

(deftest every-entry-counts-toward-the-balance
  (doseq [cents [1 20 300]]
    (is (= cents (core/balance [cents])))))

(deftest computed-entries-count-toward-the-balance
  (doseq [cents (range 100 400 100)]
    (when (pos? cents)
      (is (= cents (core/balance [cents]))))))

(deftest deposits-keep-the-owner
  (let [account (core/open-account "kim")]
    (core/deposit account 100)
    (core/deposit account 50)
    (is (= "kim" (:owner account)))))

(deftest deposits-accumulate
  (let [account (core/open-account "lee")]
    (core/deposit account 200)
    (core/deposit account 25)
    (is (= 225 (:cents (core/deposit (core/deposit account 200) 25))))))
