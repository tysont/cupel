// ABOUTME: The compiled-in languages, one module per language directory (design §8.4.2);
// ABOUTME: `javascript` and `typescript` share the `ecma` module, its queries, and its tables.
pub mod ecma;
pub mod go;
pub mod python;
pub mod rust;
