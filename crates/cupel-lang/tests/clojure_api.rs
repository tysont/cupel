// ABOUTME: The Clojure ApiSurface producer: public vars with signatures normalized for comparison, where
// ABOUTME: metadata such as type hints on a parameter vector or its parameters is no part of a signature.
mod support;

use cupel_core::syntax::{ApiItemKind, ApiSurface};
use support::{items, produce, tree};

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
