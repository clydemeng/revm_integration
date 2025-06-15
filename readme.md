# BSC-REVM Workspace

This repository glues Rust's [REVM](https://github.com/bluealloy/revm) EVM into a fork of Go-Ethereum / BSC so that Go can execute transactions through a native-speed Rust engine while still reading/writing Go's `StateDB`.

---

## 1. `revm_integration/revm_ffi_wrapper`  – Rust FFI package

### Build
```bash
# From repository root
cd revm_integration/revm_ffi_wrapper
cargo build --release
```
This produces `target/release/librevm_ffi.*` which is picked up by CGO.

### Test (unit + safety)
```bash
cd revm_integration/revm_ffi_wrapper
cargo test --all --release
```

The library has no persistent cache, so no special clean-up steps are required.

---

## 2. `forked_bsc/revm_bridge`  – Go bindings and harness

### Build / vet
```bash
cd forked_bsc
# Ensure CGO can locate the freshly-built Rust dylib via LIBRARY_PATH or LD_LIBRARY_PATH if needed
export CGO_LDFLAGS="-L$(pwd)/../revm_integration/revm_ffi_wrapper/target/release"

go vet ./revm_bridge/...
```

### Run the integration tests
The tests manipulate CGO as well as Go's test cache; always start from a clean slate:
```bash
cd forked_bsc
# Wipe previous test results so we _always_ re-run the REVM ⇆ StateDB path
go clean -testcache

# Run only the REVM bridge tests (verbose)
go test -tags=revm ./revm_bridge -v
```

If you need to force recompilation of all packages as well, append `-count=1` to the `go test` command; it runs each test once and bypasses result reuse.

---

## 3. **Placeholder – full-node smoke test**

Coming soon: scripts to launch a single-node BSC instance that links against the FFI wrapper and replays a block range.

Expected flow (not yet implemented):
```bash
# Clean both build and test caches so the node and its tests always rebuild
go clean -cache -testcache

# build the node binary (CGO enabled)
make geth-revm

# run a short chain import / tx replay
./build/bin/geth-revm --dev --revm
```

---

Maintainers: feel free to expand each section with troubleshooting notes, environment variables, etc. The headings here are meant as anchors for future detail. 