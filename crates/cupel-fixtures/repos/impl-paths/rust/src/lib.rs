// ABOUTME: `Next` for `std::vec::IntoIter<T>` and `std::vec::Drain<'_, T>`, written out twice with one body, as
// ABOUTME: itertools writes `PeekingNext` for the standard iterators it knows.
pub trait Next: Iterator {
    fn next_if(&mut self, accept: impl FnOnce(&Self::Item) -> bool) -> Option<Self::Item>;
}

impl<T> Next for ::std::vec::IntoIter<T> {
    fn next_if(&mut self, accept: impl FnOnce(&T) -> bool) -> Option<T> {
        match accept(self.as_slice().first()?) {
            true => self.next(),
            false => None,
        }
    }
}

impl<T> Next for ::std::vec::Drain<'_, T> {
    fn next_if(&mut self, accept: impl FnOnce(&T) -> bool) -> Option<T> {
        match accept(self.as_slice().first()?) {
            true => self.next(),
            false => None,
        }
    }
}
