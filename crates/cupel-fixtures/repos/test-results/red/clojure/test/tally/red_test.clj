(ns tally.red-test
  (:require [clojure.test :refer [deftest is]]
            [tally.core :as tally]))

(deftest reports-a-wrong-total
  (is (= 5 (tally/total [2 2])) "totalling 2 and 2"))
