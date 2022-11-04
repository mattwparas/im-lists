use std::iter::FromIterator;

trait ArrayLike<T>: FromIterator<T> {
    fn get(&self) -> Option<&T>;
    fn push(&self, other: T);
    fn new() -> Self;
    fn truncate(&mut self, new_len: usize);
    fn append(&mut self, other: impl IntoIterator<Item = T>);
    fn reverse(&mut self);
}
