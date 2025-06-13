# REVM ↔ Go StateDB FFI contract

This document defines the **exact memory layout** of the values that travel
between Go → C (cgo) → Rust when REVM requests state data through its
`Database` trait.  Maintaining a stable ABI is crucial: both sides must agree
on the byte-level representation.

> All structs use `#[repr(C)]` on the Rust side and the equivalent `typedef
> struct { … }` on the C side.  Go obtains raw pointers to these C structs via
> cgo and converts them into idiomatic Go types (using `unsafe` slices where
> required).

---
## 1. Primitive types

| Concept            | Rust                                  | C                                                        | Go                                   |
|--------------------|---------------------------------------|----------------------------------------------------------|--------------------------------------|
| Address            | `revm::primitives::Address` (`[u8;20]`) | `typedef struct { uint8_t bytes[20]; } Address;`         | `type Address [20]byte`              |
| 256-bit hash       | `revm::primitives::B256` (`[u8;32]`)   | `typedef struct { uint8_t bytes[32]; } Hash256;`         | `type Hash256 [32]byte`              |
| 256-bit unsigned   | `revm::primitives::U256` (`[u8;32]`)   | `typedef struct { uint8_t bytes[32]; } U256;`            | `type U256 [32]byte // big-endian`   |

*Endianness* – All numbers are **big-endian**, matching the canonical RLP and
JSON-RPC representation used by BSC/ETH.

---
## 2. Composite types

### `AccountInfo`

Rust source type: `revm::state::AccountInfo`

For FFI we only need a subset of its fields – the actual `Bytecode` is fetched
separately via `code_by_hash`.

| Field      | Rust          | C / Go notes                       |
|------------|---------------|------------------------------------|
| `balance`  | `U256`        | 32 bytes big-endian integer        |
| `nonce`    | `u64`         | little-endian in memory            |
| `codeHash` | `B256`        | 32 bytes                           |

C definition:
```c
typedef struct {
    U256   balance;     // 32 bytes
    uint64_t nonce;     // 8  bytes
    Hash256 code_hash;  // 32 bytes
} AccountInfo;          // total 72 bytes, 8-byte aligned
```

Go definition:
```go
// AccountInfo mirrors the C struct one-for-one.  Use unsafe.Pointer when
// converting, **do not** change the field order.
type AccountInfo struct {
    Balance  [32]byte // big-endian U256
    Nonce    uint64
    CodeHash [32]byte
}
```

---
## 3. Helper constants & layout tests

The Rust crate contains compile-time assertions (`assert_eq!(size_of::<T>(), …)`)
that fail the build if any of the struct sizes change.  This guards against
accidental ABI breakage when upgrading REVM or refactoring code.

See `src/statedb_types.rs` for the authoritative definitions and tests.

---
## 4. Future-proofing

* If REVM introduces new database methods that require additional data, **do not
  mutate the existing structs**. Instead, create _new_ FFI structs and bump the
  C function signatures, keeping the old versions around until the Go side has
  been migrated.
* Keep everything `#[repr(C)]` and avoid Rust enums or generics in the FFI
  layer – they do not have a stable layout across compilers. 