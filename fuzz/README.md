# DEFLATE fuzz target

Install `cargo-fuzz` and run the bounded raw-DEFLATE target from the repository
root:

```console
cargo fuzz run inflate -- -max_len=1048576
```

The fuzz crate is a separate workspace, so its dependencies and targets are
not built by the production workspace checks.
