(ns ledger.pricing-test
  (:require [clojure.test :refer [deftest is]]
            [ledger.core :as core]
            [ledger.decide :as decide]
            [ledger.pricing :as pricing]))

(deftest tally-reports-overdraft
  (let [tally (pricing/tally [5 -10 2])]
    (is (= -3 (:total tally)))
    (is (:overdrawn tally))))

(deftest parse-cents-accepts-digits
  (is (= 125 (decide/parse-cents " 125 "))))

(deftest parse-cents-rejects-blanks
  (is (thrown? IllegalArgumentException (decide/parse-cents "  "))))

(deftest mail-notifies-the-owner
  (with-redefs [pricing/log-total (fn [_] nil)]
    (let [sent (atom nil)]
      (pricing/mail (fn [owner _] (reset! sent owner)) (core/open-account "ada"))
      (is (= "ada" @sent)))))
