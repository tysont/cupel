;; ABOUTME: The fixture's build, run as `clojure -T:build compile-java`: it compiles src/java into target/classes with
;; ABOUTME: the JDK's own compiler, as tools.build's javac would, so the fixture fetches no dependency.
(ns build
  (:import (java.io File)
           (javax.tools ToolProvider)))

(defn compile-java
  "Compiles every Java source under src/java into target/classes."
  [_]
  (let [sources (->> (file-seq (File. "src/java"))
                     (filter #(.endsWith (.getName ^File %) ".java"))
                     (map #(.getPath ^File %)))
        arguments (into-array String (concat ["-d" "target/classes"] sources))
        exit (.run (ToolProvider/getSystemJavaCompiler) nil nil nil arguments)]
    (when-not (zero? exit)
      (throw (ex-info "javac failed" {:exit exit})))))
