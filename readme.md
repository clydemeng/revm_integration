# develop workflow

## revm_ffi_wrapper
1. make changes in revm_ffi_wrapper
2. build by using:
```
cargo build --release
```

## forked_bsc
### build native bsc geth
```
go run build/ci.go install ./cmd/geth

cp build/bin/geth ../perf_comparision/native_bsc_node_startup/geth

cd ../perf_comparision/native_bsc_node_startup && chmod +x bsc_performance_verification.sh && ./
```

### build bsc geth with revm ffi
```

go build -tags revm -o ./build/bin/geth ./cmd/geth

cp forked_bsc/build/bin/geth_revm perf_comparision/revm_bsc_node_startup/geth_revm

revmffi_bsc_perf_verification.sh
```

