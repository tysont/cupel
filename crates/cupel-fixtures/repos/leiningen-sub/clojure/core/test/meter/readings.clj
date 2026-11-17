(ns meter.readings
  (:require [clojure.test :refer [deftest is]]
            [meter.core :as meter]))

(deftest totals-the-readings
  (is (= 6 (meter/total [1 2 3]))))
