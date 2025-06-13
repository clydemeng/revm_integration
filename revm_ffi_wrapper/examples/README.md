# REVM vs BSC-EVM Performance Benchmarks

This directory contains comprehensive performance benchmarks comparing REVM (Rust) with BSC's Go-Ethereum EVM implementation. The benchmarks measure pure EVM execution performance using batch token transfers.

## 📁 Project Structure

```
examples1/
├── README.md                    # This file
├── BENCHMARK_RESULTS.md         # Detailed performance results
├── contracts/                   # Shared Solidity contracts
│   └── BIGA.sol                # ERC20 token with batch transfer functionality
├── bytecode/                   # Shared compiled bytecode
│   └── BIGA.bin                # Compiled BIGA contract bytecode
├── benchmark_pure_revm/        # Pure REVM (Rust) benchmark
├── benchmark_pure_goevm/       # BSC-EVM (Go) benchmark
└── benchmark_go_call_ffi/      # REVM FFI (Go→Rust) benchmark
```

## 🚀 Quick Start

### Prerequisites

1. **Rust**: Latest stable version with Cargo
2. **Go**: Version 1.19+ 
3. **Solidity Compiler**: `solc` for contract compilation
4. **REVM FFI Library**: Built in release mode

### Build REVM FFI Library

```bash
# From the project root
cargo build --release --package revm-ffi
```

### Run All Benchmarks

```bash
# 1. Pure REVM (Rust)
cd benchmark_pure_revm
cargo run --release

# 2. BSC-EVM (Go)
cd ../benchmark_pure_goevm
go run .

# 3. REVM FFI (Go→Rust)
cd ../benchmark_go_call_ffi
go run .
```

## 🔧 Running Individual Sub-Projects

### 1. Pure REVM Benchmark (`benchmark_pure_revm/`)

**Description**: Native Rust implementation using REVM directly.

**Requirements**:
- Rust toolchain (stable)
- REVM dependencies (automatically handled by Cargo)

**How to Run**:
```bash
cd benchmark_pure_revm
cargo run --release
```

**Expected Output**:
```
🚀 Pure REVM Benchmark - BIGA Token Batch Transfers
📦 Deploying BIGA contract...
💰 Minting tokens to Alice...
✅ Alice's balance: 10000000 tokens
🔄 Performing batch transfers...
⚡ Benchmark Results:
   Transfers: 50000
   Duration: ~135ms
   Transfers/sec: ~368,535
🔍 Verifying transfers...
   Recipient 1: 1 tokens
   Recipient 2: 1 tokens
   Recipient 3: 1 tokens
   Alice final balance: 9950000 tokens
✨ Benchmark completed successfully!
```

**Key Features**:
- Direct REVM API usage with `MainnetEvm`
- Zero FFI overhead
- Fastest execution time

---

### 2. BSC-EVM Benchmark (`benchmark_pure_goevm/`)

**Description**: Go implementation using BSC's Go-Ethereum EVM fork.

**Requirements**:
- Go 1.19+
- BSC dependencies (included in go.mod)

**How to Run**:
```bash
cd benchmark_pure_goevm
go run .
```

**Expected Output**:
```
🚀 Pure BSC-EVM Benchmark - BIGA Token Batch Transfers
📦 Deploying BIGA contract...
Contract deployed at: 0x5DDDfCe53EE040D9EB21AFbC0aE1BB4Dbb0BA643, gas used: 936796
💰 Minting tokens to Alice...
Transaction executed, gas used: 47306
Transaction executed, gas used: 846
✅ Alice's balance: 10000000 tokens
🔄 Performing batch transfers...
Transaction executed, gas used: 1254951376
⚡ BSC-EVM Benchmark Results:
   Transfers: 50000
   Duration: ~281ms
   Transfers/sec: ~177,920
🔍 Verifying transfers...
   Alice final balance: 9950000 tokens
✨ BSC-EVM Benchmark completed successfully!
```

**Key Features**:
- Production-ready BSC EVM implementation
- Shanghai/Cancun hard fork support
- Detailed gas usage reporting

---

### 3. REVM FFI Benchmark (`benchmark_go_call_ffi/`)

**Description**: Go application calling REVM through C FFI interface.

**Requirements**:
- Go 1.19+
- REVM FFI library (must be built first)
- CGO enabled

**Prerequisites**:
```bash
# Build REVM FFI library first (from project root)
cargo build --release --package revm-ffi
```

**How to Run**:
```bash
cd benchmark_go_call_ffi
go run .
```

**Expected Output**:
```
🚀 Pure REVM FFI Benchmark - Batch Token Transfers (BIGA)
🔧 Creating REVM instance... ✅
💰 Setting up Alice account... ✅
📦 Deploying BIGA contract... ✅ 0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643 (158µs)
💰 Minting tokens to Alice... 🔍 Transaction result: success=1, gas_used=68746
✅ Alice's balance: 10000000 tokens
🔄 Performing batch transfers...
   Alice balance before: 10000000 tokens
🔍 Transaction result: success=1, gas_used=1254977744
✅ Batch transfer transaction succeeded
⚡ REVM FFI Benchmark Results:
   Transfers: 50000
   Duration: ~137ms
   Transfers/sec: ~368,653
🔍 Verifying transfers...
   Alice final balance: 9950000 tokens
   Recipient 1: 1 tokens
   Recipient 2: 1 tokens
   Recipient 3: 1 tokens
✨ REVM FFI Benchmark completed successfully!
```

**Key Features**:
- Demonstrates FFI bridge functionality
- Minimal performance overhead compared to pure REVM
- Detailed transaction result reporting

**Troubleshooting FFI Issues**:
```bash
# If you get library not found errors:
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:../../../../target/release

# On macOS, you might need:
export DYLD_LIBRARY_PATH=$DYLD_LIBRARY_PATH:../../../../target/release
```

## 🧪 Test Scenario

Each benchmark performs the same test:
- **Contract**: BIGA token (ERC20 with batch transfers)
- **Operation**: 50,000 sequential token transfers in a single transaction
- **Amount**: 1 token per transfer (18 decimals)
- **Measurement**: Pure EVM execution time

## 📊 Performance Results

| Implementation | Transfers/sec | Duration | Performance Ratio |
|---------------|---------------|----------|-------------------|
| REVM FFI      | ~368,653     | 135.6ms  | 2.07x faster     |
| Pure REVM     | ~368,535     | 135ms    | 2.07x faster     |
| BSC-EVM       | ~177,920     | 281ms    | Baseline         |

**Key Finding**: REVM consistently outperforms BSC-EVM by approximately **2.1x** while maintaining full EVM compatibility.

## 🔧 Benchmark Details

### 1. Pure REVM (`benchmark_pure_revm/`)
- **Language**: Rust
- **Implementation**: Direct REVM API calls using `MainnetEvm`
- **Features**: Native Rust performance, zero FFI overhead

### 2. BSC-EVM (`benchmark_pure_goevm/`)
- **Language**: Go
- **Implementation**: BSC's Go-Ethereum EVM fork
- **Features**: Shanghai/Cancun hard fork support, production-ready

### 3. REVM FFI (`benchmark_go_call_ffi/`)
- **Language**: Go calling Rust
- **Implementation**: C FFI bridge to REVM
- **Features**: Demonstrates minimal FFI overhead

## 🔄 Contract Compilation

To recompile the BIGA contract:

```bash
cd contracts
solc --bin --overwrite -o . BIGA.sol
cp BIGA.bin ../bytecode/
```

## 📋 State Verification

All benchmarks verify correct execution by checking:
- Alice's token balance (10M → 9.95M after 50k transfers)
- Recipient balances (each receives 1 token)
- Proper event emission
- Gas consumption consistency

## 🛠 Troubleshooting

### Common Issues

1. **FFI Library Not Found**
   ```bash
   # Ensure REVM FFI is built
   cargo build --release --package revm-ffi
   ```

2. **Go Module Issues**
   ```bash
   # Clean and rebuild Go modules
   go mod tidy
   go clean -modcache
   ```

3. **Solidity Compilation**
   ```bash
   # Install solc if missing
   npm install -g solc
   ```

### Gas Limit Errors
If you encounter out-of-gas errors, the benchmarks are configured with appropriate gas limits:
- REVM: 2B gas limit
- BSC-EVM: 2B gas limit
- FFI: 2B gas limit

## 📈 Performance Analysis

The benchmarks demonstrate:
1. **REVM's Superior Performance**: 2.1x faster than Go-Ethereum
2. **Minimal FFI Overhead**: REVM FFI performs identically to pure REVM
3. **EVM Compatibility**: All implementations handle the same contract correctly
4. **Consistent Gas Usage**: Similar gas consumption across implementations

## 🎯 Use Cases

These benchmarks are valuable for:
- **Performance Evaluation**: Comparing EVM implementations
- **Architecture Decisions**: Choosing between native Rust vs FFI integration
- **Optimization Research**: Understanding EVM execution bottlenecks
- **Integration Testing**: Validating REVM FFI functionality

## 📚 Additional Resources

- [BENCHMARK_RESULTS.md](./BENCHMARK_RESULTS.md) - Detailed performance analysis
- [REVM Documentation](https://github.com/bluealloy/revm)
- [BSC Documentation](https://docs.bnbchain.org/)

## 🤝 Contributing

To add new benchmarks or improve existing ones:
1. Follow the existing directory structure
2. Use the shared `contracts/` and `bytecode/` directories
3. Update this README and BENCHMARK_RESULTS.md
4. Ensure consistent test scenarios across implementations 