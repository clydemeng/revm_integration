//! FFI-friendly representations of types used by the REVM StateDB callbacks.
//!
//! These types are the only "wire" format shared between Go <-> C (CGO) <-> Rust.
//! They MUST remain stable – **do not** change their memory layout without bumping
//! the crate major version and updating the Go side.
//!
//! All structs deliberately use `#[repr(C)]` and plain value fields so that they
//! can cross the FFI boundary without undefined behaviour.
//!
//! NOTE: Complex/heap-based fields like `Bytecode` are _not_ part of the FFI
//! footprint.  The Rust side will request the contract code on demand via
//! `code_by_hash`, so the account's `code` itself does **not** have to cross the
//! boundary.

use core::mem::size_of;

/// 160-bit Ethereum address (20 raw bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FFIAddress {
    pub bytes: [u8; 20],
}

/// 256-bit hash (Keccak-256, storage slot, etc.).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FFIHash {
    pub bytes: [u8; 32],
}

/// 256-bit unsigned integer (big-endian byte order).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FFIU256 {
    pub bytes: [u8; 32],
}

/// Sub-set of `revm::state::AccountInfo` that is required by the database trait.
///
/// * `balance`   – The account's ETH/BSC balance.
/// * `nonce`     – Transaction nonce.
/// * `code_hash` – Keccak-256 hash of the account's bytecode.
///
/// The actual bytecode does **not** cross the FFI boundary.  It will be
/// requested separately using `code_by_hash`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FFIAccountInfo {
    pub balance: FFIU256,
    pub nonce: u64,
    pub code_hash: FFIHash,
}

// ---------------------------------------------------------------------------
//  Compile-time layout assertions – these act as unit tests and prevent silent
//  ABI breakage.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_layout_sizes() {
        assert_eq!(size_of::<FFIAddress>(), 20, "FFIAddress must be 20 bytes");
        assert_eq!(size_of::<FFIHash>(), 32, "FFIHash must be 32 bytes");
        assert_eq!(size_of::<FFIU256>(), 32, "FFIU256 must be 32 bytes");
        assert_eq!(size_of::<FFIAccountInfo>(), 72, "FFIAccountInfo is 32 + 8 + 32 = 72 bytes");
    }
} 