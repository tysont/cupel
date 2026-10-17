(ns tally.core-test
  (:require [clojure.test :refer [deftest is]]
            [tally.core :as tally]))

(deftest totals-a-list
  (is (= 6 (tally/total [1 2 3]))))

(deftest totals-nothing
  (is (zero? (tally/total []))))
