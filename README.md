# rd
`rd` is a command-line interface to Rust documentation.

It is currently in the very early stages. It can provide function signatures for functions that live directly inside modules (not part of a `trait` or `impl`).

## Building
Build the crate:
```
cargo build
```
Generate documentation for all crates in `~/.cargo/registry/src`:
```
cargo run -- -g all
```
Or generate documentation for the specified crate source directory:
```
cargo run -- -g ~/build/rd/
```

The generated documentation currently lives in `~/.cargo/registry/doc`.

Currently a name without a path can be used to search though all crates, or a fully qualified path can be provided, to search only inside that crate's module:
```
cargo run get_fn_file
cargo run rd::store::get_fn_file
```

This provides:

```
= rd::store::get_fn_file

(from crate rd-0.1.0)
=== rd::store::get_fn_file()
------------------------------------------------------------------------------
  fn get_fn_file(path: &PathBuf, fn_doc: &FnDoc) -> PathBuf

------------------------------------------------------------------------------

Description will go here.
```

## Notes
The design and output are heavily borrowed from `ri`. Many things still need to be done before `rd` is truly useful, so contribution is welcomed.

## TODO
- Docstrings (not currently available in the AST)
- Documentation for crates, traits, constants, modules, structs, etc.
- Documentation for `std`
- Tests
- Fuzzy matching
- Searching by type signature
- Filtering by unsafety/trait
- Showing lifetime information for module paths
- Documenting generics
- Handling non-standard crate entry points
- Probably many other things.
