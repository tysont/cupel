(ns counter.core-test
  (:require [clojure.java.io :as io]
            [clojure.test :refer [deftest is]]
            [counter.core :as counter]))

(defn- trace
  "The trace file name in the directory CUPEL_TRACE_DIR names, when it is set."
  [name]
  (some-> (System/getenv "CUPEL_TRACE_DIR") (io/file name) str))

(deftest counts-then-finishes
  (let [c (-> (counter/counter (trace "counts.jsonl"))
              counter/inc-count
              counter/inc-count
              counter/finish)]
    (is (= 2 (:count c)))))

(deftest bumps
  (let [c (-> (counter/counter (trace "bumps.jsonl"))
              counter/inc-count
              counter/bump)]
    (is (= 3 (:count c)))))
