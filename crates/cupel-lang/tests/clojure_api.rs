// ABOUTME: The Clojure ApiSurface producer: public vars with signatures normalized for comparison, where metadata
// ABOUTME: is no part of a signature, and vars outside the project's published source paths are internal (ADR-215).
mod support;

use cupel_core::syntax::{ApiItemKind, ApiSurface};
use support::{exposure, items, produce, tree};

use ApiItemKind::Function;

fn item(path: &str, signature: &str) -> (String, ApiItemKind, String) {
    (path.to_owned(), Function, signature.to_owned())
}

/// The `ApiSurface` items of `source` as `src/a/core.clj`.
fn surface(source: &str) -> Vec<(String, ApiItemKind, String)> {
    let dir = tree(&[("src/a/core.clj", source)]);
    let api: ApiSurface = produce("clojure", dir.path());
    items(&api, "src/a/core.clj")
}

#[test]
fn type_hints_and_metadata_on_a_parameter_vector_or_its_parameters_are_no_part_of_a_signature() {
    let source = "(ns a.core)\n\
         (defn plain [x] x)\n\
         (defn returns ^String [x] x)\n\
         (defn tagged ^{:tag String} [x] x)\n\
         (defn flagged ^:once [x] x)\n\
         (defn legacy #^String [x] x)\n\
         (defn params [^String s ^{:tag long} n & ^Object more] s)\n\
         (defn documented \"Doc.\" {:added \"1.0\"} ^String [x] x)\n";
    assert_eq!(
        surface(source),
        [
            item("a.core/plain", "[_]"),
            item("a.core/returns", "[_]"),
            item("a.core/tagged", "[_]"),
            item("a.core/flagged", "[_]"),
            item("a.core/legacy", "[_]"),
            item("a.core/params", "[_ _ &_]"),
            item("a.core/documented", "[_]"),
        ]
    );
}

/// clj-commons/clj-yaml's shape: `deps.edn` publishes `src/clojure`; the dev scripts on
/// `bb.edn`'s paths, an alias's extra path, and a path only an alias names are no API.
#[test]
fn a_var_is_api_only_under_the_paths_deps_edn_publishes() {
    let deps = "{:paths [\"src/clojure\" \"resources\"] ; :paths [\"script\"]\n \
                :aliases {:dev {:extra-paths [\"dev\"]}\n \
                          :legacy {:paths [\"build\"]}}}\n";
    let paths = exposure(
        "clojure",
        &[
            ("deps.edn", deps),
            ("bb.edn", "{:paths [\"script\" \"build\"]}\n"),
            ("build/build.clj", "(ns build)\n(defn jar [_] nil)\n"),
            ("dev/user.clj", "(ns user)\n(defn reload [] nil)\n"),
            (
                "script/publish.clj",
                "(ns publish)\n(defn -main [& _] nil)\n",
            ),
            (
                "src/clojure/clj_yaml/core.clj",
                "(ns clj-yaml.core)\n(defn parse-string [s] s)\n",
            ),
        ],
    );
    assert_eq!(
        paths,
        [
            "!build/jar",
            "!user/reload",
            "!publish/-main",
            "clj-yaml.core/parse-string",
        ]
    );
}

/// Without `:paths` a `deps.edn` publishes `src`, as tools.deps does; a Leiningen project
/// publishes its top-level `:source-paths` (`src` without them), not a profile's; the nearest
/// manifest above a file is its project's.
#[test]
fn deps_edn_defaults_to_src_and_leiningen_publishes_its_source_paths() {
    let lein = "(defproject lib \"0.1.0\"\n  :source-paths [\"src/clojure\"]\n  \
                :profiles {:dev {:source-paths [\"dev\"]}})\n";
    let paths = exposure(
        "clojure",
        &[
            ("deps.edn", "{:deps {}}\n"),
            ("lib/project.clj", lein),
            ("lib/dev/lib/user.clj", "(ns lib.user)\n(defn go [] nil)\n"),
            (
                "lib/src/clojure/lib/core.clj",
                "(ns lib.core)\n(defn run [] nil)\n",
            ),
            ("plain/project.clj", "(defproject plain \"0.1.0\")\n"),
            (
                "plain/src/plain/core.clj",
                "(ns plain.core)\n(defn run [] nil)\n",
            ),
            ("script/tool.clj", "(ns tool)\n(defn run [] nil)\n"),
            ("src/a/core.clj", "(ns a.core)\n(defn f [] nil)\n"),
        ],
    );
    assert_eq!(
        paths,
        [
            "!lib.user/go",
            "lib.core/run",
            "plain.core/run",
            "!tool/run",
            "a.core/f",
        ]
    );
}

/// A tree with neither `deps.edn` nor `project.clj` above a file names no published paths, so
/// its public vars are all API, as before.
#[test]
fn without_a_project_manifest_every_public_var_is_api() {
    let paths = exposure(
        "clojure",
        &[
            ("bb.edn", "{:paths [\"script\"]}\n"),
            ("script/tool.clj", "(ns tool)\n(defn run [] nil)\n"),
            ("src/a/core.clj", "(ns a.core)\n(defn f [] nil)\n"),
        ],
    );
    assert_eq!(paths, ["tool/run", "a.core/f"]);
}
