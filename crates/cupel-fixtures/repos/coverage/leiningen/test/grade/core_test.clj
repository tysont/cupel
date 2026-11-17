(ns grade.core-test
  (:require [clojure.test :refer [deftest is]]
            [grade.core :as grade]))

(deftest totals-a-list
  (is (= 6 (grade/total [1 2 3]))))

(deftest classifies-passing-scores
  (is (= "A" (grade/classify 95)))
  (is (= "pass" (grade/classify 60))))
