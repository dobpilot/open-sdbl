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

The fuzz crate is a separate workspace, so its dependencies and targets are
not built by the production workspace checks.
