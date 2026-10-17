;; ABOUTME: Cupel's clojure.test runner (ADR-109): loads the test namespaces on the classpath, runs them or the tests
;; ABOUTME: and files named after the report path, and writes clojure.test's JUnit XML to the report file.
;;
;; clojure -M:test <this file> <report> [--list] [--seed N] [<namespace>/<var> | <file>]...
;;
;; With no tests or files named, every `*_test.clj(c)` namespace in a classpath directory runs. Named
;; tests run in the order given, grouped into one suite per run of the same namespace; `--seed` shuffles
;; them with that seed first. `--list` prints each test's ID instead of running anything. The exit code is
;; 0 when every test passed, else 1.
(ns cupel.test-runner
  (:require [clojure.java.io :as io]
            [clojure.string :as str]
            [clojure.test :as test]
            [clojure.test.junit :as junit])
  (:import (java.io File PushbackReader)
           (java.util ArrayList Collections Random)))

(defn- classpath-dirs []
  (->> (str/split (System/getProperty "java.class.path") (re-pattern (str File/pathSeparator)))
       (map io/file)
       (filter #(.isDirectory ^File %))))

(defn- test-file? [^File file]
  (and (.isFile file) (re-find #"_test\.cljc?$" (.getName file))))

(defn- namespace-of
  "The namespace a file's `ns` form names, or nil."
  [file]
  (with-open [reader (PushbackReader. (io/reader file))]
    (binding [*read-eval* false]
      (let [form (read {:eof nil :read-cond :allow} reader)]
        (when (and (seq? form) (= 'ns (first form)))
          (second form))))))

(defn- test-namespaces []
  (->> (classpath-dirs)
       (mapcat file-seq)
       (filter test-file?)
       (sort-by #(.getPath ^File %))
       (keep namespace-of)
       distinct))

(defn- test-vars
  "The vars of `ns` that hold tests, in the order they are defined."
  [ns]
  (require ns)
  (->> (vals (ns-interns ns))
       (filter (comp :test meta))
       (sort-by (comp (juxt :line :column) meta))))

(defn- selected-vars [selectors]
  (if (empty? selectors)
    (mapcat test-vars (test-namespaces))
    (mapcat (fn [selector]
              (if (re-find #"\.cljc?$" selector)
                (test-vars (namespace-of (io/file selector)))
                (let [id (symbol selector)]
                  (require (symbol (namespace id)))
                  [(find-var id)])))
            selectors)))

(defn- shuffled [vars seed]
  (let [list (ArrayList. ^java.util.Collection vars)]
    (Collections/shuffle list (Random. seed))
    (vec list)))

(defn- id-of [v]
  (let [{:keys [ns name]} (meta v)]
    (str (ns-name ns) "/" name)))

(defn- run-suites
  "Runs `vars`, one suite per run of the same namespace, with JUnit output to `report`; the
  counters."
  [vars report]
  (with-open [writer (io/writer report)]
    (binding [test/*test-out* writer
              test/*report-counters* (ref test/*initial-report-counters*)]
      (junit/with-junit-output
        (doseq [suite (partition-by #(:ns (meta %)) vars)]
          (let [ns (:ns (meta (first suite)))]
            (test/do-report {:type :begin-test-ns :ns ns})
            (test/test-vars suite)
            (test/do-report {:type :end-test-ns :ns ns}))))
      @test/*report-counters*)))

(defn- options [args]
  (loop [args args, parsed {:selectors []}]
    (case (first args)
      nil parsed
      "--list" (recur (rest args) (assoc parsed :list true))
      "--seed" (recur (drop 2 args) (assoc parsed :seed (Long/parseLong (second args))))
      (recur (rest args) (update parsed :selectors conj (first args))))))

(let [[report & args] *command-line-args*
      {:keys [list seed selectors]} (options args)
      vars (cond-> (vec (selected-vars selectors)) seed (shuffled seed))]
  (if list
    (do (doseq [v vars] (println (id-of v)))
        (shutdown-agents)
        (System/exit 0))
    (let [{:keys [fail error]} (run-suites vars report)]
      (shutdown-agents)
      (System/exit (if (zero? (+ fail error)) 0 1)))))
