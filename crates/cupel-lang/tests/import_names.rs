// ABOUTME: The names each language's import edges carry (ADR-133): the item a Rust path takes, a Python
// ABOUTME: `from` import's name, and the bindings of a JavaScript import or re-export, `default` included.
mod support;

use support::assert_imported_names;

/// A Rust path names the item it takes from the module it resolves to; a Python `from` import
/// names what it takes unless that is a submodule or a wildcard; a JavaScript import or
/// re-export names its bindings as the module spells them, and `default` for a default import.
#[test]
fn each_edge_names_what_its_statement_takes_from_the_module() {
    assert_imported_names(
        "rust",
        &[
            ("Cargo.toml", "[package]\nname = \"app\"\n"),
            ("src/lib.rs", "mod a;\nmod b;\n"),
            (
                "src/a.rs",
                "use crate::b::{self, B, C as D};\nuse crate::b::*;\nuse super::b::E::V;\n",
            ),
            (
                "src/b.rs",
                "pub struct B;\npub struct C;\npub enum E { V }\n",
            ),
        ],
        "src/a.rs",
        &[
            ("crate::b", &[]),
            ("crate::b::B", &["B"]),
            ("crate::b::C", &["C"]),
            ("crate::b::*", &[]),
            ("super::b::E::V", &["E"]),
        ],
    );
    assert_imported_names(
        "python",
        &[
            ("pkg/__init__.py", ""),
            (
                "pkg/a.py",
                "from .b import B, C as D\nfrom . import b\nfrom .b import *\nimport pkg.b\n",
            ),
            ("pkg/b.py", "B = 1\nC = 2\n"),
        ],
        "pkg/a.py",
        &[
            (".b.B", &["B"]),
            (".b.C", &["C"]),
            (".b", &[]),
            (".b.*", &[]),
            ("pkg.b", &[]),
        ],
    );
    assert_imported_names(
        "javascript",
        &[
            ("package.json", "{\"name\": \"app\"}"),
            (
                "src/a.js",
                "import d, { b, c as e } from './b.js';\nimport * as ns from './b.js';\nexport { f } from './b.js';\nconst r = require('./b.js');\nimport './b.js';\n",
            ),
            (
                "src/b.js",
                "export default 1;\nexport const b = 2, c = 3, f = 4;\n",
            ),
        ],
        "src/a.js",
        &[
            ("./b.js", &["default", "b", "c"]),
            ("./b.js", &[]),
            ("./b.js", &["f"]),
            ("./b.js", &[]),
            ("./b.js", &[]),
        ],
    );
}
