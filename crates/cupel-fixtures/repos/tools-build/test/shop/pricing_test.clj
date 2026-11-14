;; ABOUTME: clojure.test cases for the tools-build fixture's pricing.
;; ABOUTME: One test, one assertion, so a run that reports it is plainly Cupel's.
(ns shop.pricing-test
  (:require [clojure.test :refer [deftest is]]
            [shop.pricing :as pricing]))

(deftest discount-takes-the-percent-off
  (is (= 90 (pricing/discount 100 10))))
