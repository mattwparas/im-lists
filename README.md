# im-lists

![Actions Status](https://github.com/mattwparas/im-lists/workflows/Build/badge.svg) [![Coverage Status](https://coveralls.io/repos/github/mattwparas/im-lists/badge.svg?branch=master)](https://coveralls.io/github/mattwparas/im-lists?branch=master) [![Crate Status](https://img.shields.io/crates/v/im-lists.svg)](https://crates.io/crates/im-lists) [![Docs Status](https://docs.rs/im-lists/badge.svg)](https://docs.rs/im-lists/0.1.0/im_lists/)

An implementation of a persistent unrolled linked list and vlist. This linked list is implemented with a backing of either `Arc` or `Rc`, for single or multi-threaded environments. The single threaded list can be found as a `List`, and the thread-safe implementation can be found as a `SharedList`. It is generic over smart pointer - so if you would like to use this with a custom smart pointer (i.e. something like a `Gc`) then you can do so.

An unrolled linked list is a linked list where each node contains a vector of elements. While the algorithmic complexity is the same as a normal naive linked list, storing elements in vectors improves cache locality and also gives practical speed ups on common operations. A Vlist is like an unrolled linked list, however the vector capacity in each node grows exponentially. This also means that operations that need to iterate over nodes run in `O(log(n))` time.

## License

Licensed under either of

* Apache License, Version 2.0
  ([LICENSE-APACHE](https://github.com/mattwparas/im-lists/blob/master/LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
  ([LICENSE-MIT](https://github.com/mattwparas/im-lists/blob/master/LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](https://github.com/mattwparas/im-lists/blob/master/CONTRIBUTING.md).
