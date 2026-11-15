;; ABOUTME: clojure.test tests of meter.core under Leiningen's default test path, `test`, which `lein test`
;; ABOUTME: runs.
(ns meter.core-test
  (:require [clojure.test :refer [deftest is]]
            [meter.core :as meter]))

(deftest totals-the-readings
  (is (= 6 (meter/total [1 2 3]))))
