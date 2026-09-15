# Fuzz targets

Install `cargo-fuzz` and run the bounded raw-DEFLATE target from the repository
root:

```console
cargo fuzz run inflate -- -max_len=1048576
```

Compile arbitrary UTF-8 source against a fixed synthetic metadata snapshot:

```console
cargo fuzz run compile_query -- -max_len=1048576
```

The snapshot lives in `src/lib.rs` and describes a catalog with an ordinary
field, a reference field pointing at a second catalog, and a tabular section,
so generated source can reach dereference and tabular-section code generation
instead of stopping at name resolution. A unit test there compiles one query of
each shape, which fails loudly whenever the fixture stops resolving.

The fuzz crate is a separate workspace, so its dependencies and targets are not
built by the production workspace checks. CI still builds it and runs that unit
test — never an unbounded campaign:

```console
cargo check --manifest-path fuzz/Cargo.toml --all-targets
cargo test --manifest-path fuzz/Cargo.toml
```
