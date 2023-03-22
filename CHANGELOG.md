# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]


## [0.3.0] - 2022-11-03
### Added
- Changed the internal representation to use the new GAT feature. No longer need to do so many layers of indirection
- Added the capacity as a const generic. You can now pick the maximum size of any individual node in the unrolled linked list