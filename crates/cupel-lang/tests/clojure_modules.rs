// ABOUTME: The Clojure ModuleGraph producer for namespaces loaded by a qualified symbol: a registry that
// ABOUTME: `requiring-resolve`s `ns/var`, directly or through a macro, depends on that namespace (P9.9 group 3, cljfx).
mod support;

use cupel_core::syntax::ModuleGraph;
use support::{EdgeRow, edge_rows, produce, row, tree};

/// cljfx's registry: `(lazy-load cljfx.fx.button/lifecycle)` expands to `(requiring-resolve
/// 'cljfx.fx.button/lifecycle)`, which loads the namespace on first use, so nothing `:require`s it.
/// A quoted symbol is data (cljfx names its extension lifecycles `'cljfx.api/ext-watcher`) unless
/// `requiring-resolve` or `resolve` is given it.
const REGISTRY: &str = r"(ns app.fx
  (:require [app.widgets.base :as widgets.base]
            [clojure.string :as str]))

(defmacro lazy-load [sym]
  `(delay @(requiring-resolve '~sym)))

(def registry
  {:button (lazy-load app.widgets.button/lifecycle)
   :label (delay @(requiring-resolve 'app.widgets.label/lifecycle))
   :base widgets.base/lifecycle
   :name (str/join [app.fx/registry clojure.core/inc])
   :label-name 'app.widgets.other/name})
";

/// The namespaces the registry names: one it requires, two it loads lazily, one only quoted.
const WIDGETS: [(&str, &str); 4] = [
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
    (
        "src/app/widgets/other.clj",
        "(ns app.widgets.other)\n(def name 3)\n",
    ),
];

/// The module graph's edges over the registry and its widgets, in edge order.
fn registry_edges() -> Vec<EdgeRow> {
    let mut files = vec![
        ("deps.edn", "{:paths [\"src\"]}\n"),
        ("src/app/fx.clj", REGISTRY),
    ];
    files.extend(WIDGETS);
    let graph: ModuleGraph = produce("clojure", tree(&files).path());
    edge_rows(&graph)
}

#[test]
fn a_qualified_symbol_naming_a_namespace_of_the_project_depends_on_it() {
    let edges = registry_edges();
    let from = "src/app/fx.clj";
    assert_eq!(edges.len(), 4, "{edges:?}");
    assert_eq!(
        edges[0],
        row(
            ".",
            (
                from,
                2,
                "app.widgets.base",
                "src/app/widgets/base.clj",
                false
            )
        )
    );
    assert_eq!(
        edges[1],
        row(".", (from, 3, "clojure.string", "external:clojure", false))
    );
    assert_eq!(
        edges[2],
        row(
            ".",
            (
                from,
                9,
                "app.widgets.button/lifecycle",
                "src/app/widgets/button.clj",
                false
            )
        )
    );
    assert_eq!(
        edges[3],
        row(
            ".",
            (
                from,
                10,
                "app.widgets.label/lifecycle",
                "src/app/widgets/label.clj",
                false
            )
        )
    );
}
