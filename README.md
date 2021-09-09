# im-lists

![Actions Status](https://github.com/mattwparas/im-lists/workflows/Build/badge.svg) [![Coverage Status](https://coveralls.io/repos/github/mattwparas/im-lists/badge.svg?branch=master)](https://coveralls.io/github/mattwparas/im-lists?branch=master)

An implementation of a persistent unrolled linked list. This linked list is implemented with a backing of either `Arc` or `Rc`, for single or multi-threaded environments. The single threaded list can be found as a `List`, and the thread-safe implementation can be found as a `SharedList`.

An unrolled linked list is a linked list where each node contains a vector of elements. While the algorithmic complexity is the same as a normal naive linked list, storing elements in vectors improves cache locality and also gives practical speed ups on common operations.

## License

Licensed under either of

* Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

See [CONTRIBUTING.md](CONTRIBUTING.md).
