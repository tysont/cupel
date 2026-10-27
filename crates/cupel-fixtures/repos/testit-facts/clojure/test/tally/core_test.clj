;; ABOUTME: Tests in cljfx's style: testit's `fact` and `facts` assert with their arrows inside clojure.test's
;; ABOUTME: `deftest`; the last test asserts nothing.
(ns tally.core-test
  (:require [clojure.test :refer :all]
            [testit.core :refer :all]
            [tally.core :as core]))

(deftest totals-are-sums
  (let [total (core/total [1 2])
        _ (fact total => 3)]
    (facts (core/total []) => 0
           (core/total [1]) => 1)))

(deftest negative-counts-throw
  (fact (core/total [-1]) =throws=> clojure.lang.ExceptionInfo))

(deftest totals-print
  (println (core/total [1])))
