# Architecture

How the code is structured, and why. Update this file in the same change as
any structural change.

## Today

photography-workbench is one Rust crate (edition 2024) with a single
binary, `src/main.rs`. There is no library and there are no modules yet.

## As the code grows

When the code outgrows one file, this page gains a table of modules — one
row each, saying what the module owns — and a description of how data
moves between them. A change that adds a module, or moves data between
modules, updates both.
