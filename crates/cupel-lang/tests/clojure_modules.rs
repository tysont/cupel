// ABOUTME: The Clojure ModuleGraph producer for namespaces loaded by a qualified symbol: a registry that
// ABOUTME: `requiring-resolve`s `ns/var`, directly or through a macro, depends on that namespace (P9.9 group 3, cljfx).
mod support;

use cupel_core::syntax::ModuleGraph;
use support::{edge_rows, produce, tree, under};

/// cljfx's registry: `(lazy-load cljfx.fx.button/lifecycle)` expands to `(requiring-resolve
/// 'cljfx.fx.button/lifecycle)`, which loads the namespace on first use, so nothing `:require`s it.
const REGISTRY: &str = r"(ns app.fx
  (:require [app.widgets.base :as widgets.base]
            [clojure.string :as str]))

(defmacro lazy-load [sym]
  `(delay @(requiring-resolve '~sym)))

(def registry
  {:button (lazy-load app.widgets.button/lifecycle)
   :label (delay @(requiring-resolve 'app.widgets.label/lifecycle))
   :base widgets.base/lifecycle
   :name (str/join [app.fx/registry clojure.core/inc])})
";

#[test]
fn a_qualified_symbol_naming_a_namespace_of_the_project_depends_on_it() {
    let dir = tree(&[
        ("deps.edn", "{:paths [\"src\"]}\n"),
        ("src/app/fx.clj", REGISTRY),
        (
            "src/app/widgets/base.clj",
            "(ns app.widgets.base)\n(def lifecycle 0)\n",
        ),
        (
            "src/app/widgets/button.clj",
            "(ns app.widgets.button)\n(def lifecycle 1)\n",
        ),
        (
            "src/app/widgets/label.clj",
            "(ns app.widgets.label)\n(def lifecycle 2)\n",
        ),
    ]);
    let graph: ModuleGraph = produce("clojure", dir.path());
    assert_eq!(
        edge_rows(&graph),
        under(
            ".",
            &[
                (
                    "src/app/fx.clj",
                    2,
                    "app.widgets.base",
                    "src/app/widgets/base.clj",
                    false
                ),
                (
                    "src/app/fx.clj",
                    3,
                    "clojure.string",
                    "external:clojure",
                    false
                ),
                (
                    "src/app/fx.clj",
                    9,
                    "app.widgets.button/lifecycle",
                    "src/app/widgets/button.clj",
                    false
                ),
                (
                    "src/app/fx.clj",
                    10,
                    "app.widgets.label/lifecycle",
                    "src/app/widgets/label.clj",
                    false
                ),
            ]
        )
    );
}
