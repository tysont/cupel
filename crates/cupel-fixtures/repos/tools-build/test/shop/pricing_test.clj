;; ABOUTME: clojure.test cases for the tools-build fixture's pricing.
;; ABOUTME: One assertion per test, so a run that reports them is plainly Cupel's.
(ns shop.pricing-test
  (:require [clojure.test :refer [deftest is]]
            [shop.pricing :as pricing]))

(deftest discount-takes-the-percent-off
  (is (= 90 (pricing/discount 100 10))))

(deftest in-cents-rounds-to-whole-cents
  (is (= 1050 (pricing/in-cents 10.5))))
