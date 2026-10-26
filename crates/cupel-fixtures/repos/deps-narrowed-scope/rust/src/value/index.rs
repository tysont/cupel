// ABOUTME: A module that declares only a trait and imports two modules: abstract, with one importer outside
// ABOUTME: a scope narrowed to this file (instability 2/3, distance 0.67, under the 0.7 limit).
use super::Value;
use crate::map::Map;

pub trait Index {
    fn index_into<'v>(&self, value: &'v Value) -> Option<&'v Value>;

    fn index_map<'m>(&self, map: &'m Map) -> Option<&'m Map>;
}
