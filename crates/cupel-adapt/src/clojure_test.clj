;; ABOUTME: Cupel's clojure.test runner (ADR-109, ADR-239): loads the test namespaces, runs them or the tests and files
;; ABOUTME: named after the report path, and writes clojure.test's JUnit XML to the report file.
;;
;; clojure -M:test <this file> <report> [--list] [--seed N] [--test-dir DIR]... [<namespace>/<var> | <file>]...
;;
;; With no tests or files named, every `*_test.clj(c)` namespace in a classpath directory runs, or, given
;; `--test-dir`, every namespace below those directories, as `lein test` runs its `:test-paths`. Named
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

(defn- source-file? [^File file]
  (and (.isFile file) (re-find #"\.cljc?$" (.getName file))))

(defn- test-namespaces
  "The namespaces of the test files on the classpath, or of every file below `test-dirs`."
  [test-dirs]
  (->> (if (seq test-dirs) (map io/file test-dirs) (classpath-dirs))
       (mapcat file-seq)
       (filter (if (seq test-dirs) source-file? test-file?))
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

(defn- selected-vars [test-dirs selectors]
  (if (empty? selectors)
    (mapcat test-vars (test-namespaces test-dirs))
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
  (loop [args args, parsed {:selectors [], :test-dirs []}]
    (case (first args)
      nil parsed
      "--list" (recur (rest args) (assoc parsed :list true))
      "--test-dir" (recur (drop 2 args) (update parsed :test-dirs conj (second args)))
      "--seed" (recur (drop 2 args) (assoc parsed :seed (Long/parseLong (second args))))
      (recur (rest args) (update parsed :selectors conj (first args))))))

(defn -main
  "Runs or lists the selected tests, writing the report; exits 0 only when every test passed."
  [report & args]
  (let [{:keys [list seed selectors test-dirs]} (options args)
        vars (cond-> (vec (selected-vars test-dirs selectors)) seed (shuffled seed))]
    (if list
      (do (doseq [v vars] (println (id-of v)))
          (shutdown-agents)
          (System/exit 0))
      (let [{:keys [fail error]} (run-suites vars report)]
        (shutdown-agents)
        (System/exit (if (zero? (+ fail error)) 0 1))))))

(apply -main *command-line-args*)
