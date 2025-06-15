//! `GoDatabase` – REVM `Database` implementation backed by Go StateDB via FFI callbacks.
//!
//! This bridges the `re_state_*` C functions (exported from the Go side) with
//! REVM's `Database`/`DatabaseRef` traits.  A single `GoDatabase` just wraps an
//! opaque handle (`usize`) that Go gives us.  All heavy lifting is delegated
//! to the callbacks.

use crate::statedb_types::{FFIAccountInfo, FFIAddress, FFIHash, FFIU256};
use libc::free;
use revm::bytecode::Bytecode;
use revm::database_interface::{Database, DatabaseRef, DBErrorMarker};
use revm::primitives::{Address, Bytes, StorageKey, StorageValue, B256, U256};
use revm::state::AccountInfo;
use std::ffi::c_void;
use std::ptr;
use std::{error::Error, fmt};
use core::sync::atomic::{AtomicUsize, Ordering};
use revm::state::Account;
use std::collections::HashMap;
use revm::database_interface::DatabaseCommit;

#[cfg(test)]
pub static TEST_LAST_HANDLE: AtomicUsize = AtomicUsize::new(0);

/// Type alias for the error we bubble up.  We keep it simple for now – every
/// failure returns a descriptive string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoDBError(pub String);

impl fmt::Display for GoDBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for GoDBError {}

impl DBErrorMarker for GoDBError {}

/// Opaque database that forwards requests to Go.
#[derive(Clone, Copy, Debug)]
pub struct GoDatabase {
    handle: usize,
}

impl GoDatabase {
    /// Safety: `handle` must be a valid value previously obtained from the Go
    /// side via `NewStateDB`.  No further lifetime guarantees are made.
    pub fn new(handle: usize) -> Self {
        Self { handle }
    }

    fn address_to_ffi(addr: Address) -> FFIAddress {
        let mut out = FFIAddress { bytes: [0u8; 20] };
        out.bytes.copy_from_slice(addr.as_slice());
        out
    }

    fn hash_to_ffi(h: B256) -> FFIHash {
        FFIHash { bytes: h.0 }
    }

    fn ffi_u256_to_u256(u: FFIU256) -> U256 {
        U256::from_be_bytes(u.bytes)
    }

    fn ffi_hash_to_b256(h: FFIHash) -> B256 {
        B256::from_slice(&h.bytes)
    }

    fn u256_to_ffi_hash(value: U256) -> FFIHash {
        FFIHash { bytes: value.to_be_bytes() }
    }

    fn u256_to_ffi_u256(value: U256) -> FFIU256 {
        FFIU256 { bytes: value.to_be_bytes() }
    }
}

// ---------------------------------------------------------------------------
//  FFI declarations
// ---------------------------------------------------------------------------

extern "C" {
    fn re_state_basic(handle: usize, addr: FFIAddress, out_info: *mut FFIAccountInfo) -> i32;
    fn re_state_storage(handle: usize, addr: FFIAddress, slot: FFIHash, out_val: *mut FFIU256) -> i32;
    fn re_state_block_hash(handle: usize, number: u64, out_hash: *mut FFIHash) -> i32;
    fn re_state_code(
        handle: usize,
        code_hash: FFIHash,
        out_ptr: *mut *mut u8,
        out_len: *mut u32,
    ) -> i32;
    fn re_state_set_basic(handle: usize, addr: FFIAddress, info: FFIAccountInfo) -> i32;
    fn re_state_set_storage(handle: usize, addr: FFIAddress, slot: FFIHash, val: FFIU256) -> i32;
}

// ---------------------------------------------------------------------------
//  Helper – convert raw FFIAccountInfo into REVM AccountInfo
// ---------------------------------------------------------------------------

fn ffi_account_to_revm(acc: &FFIAccountInfo) -> AccountInfo {
    let balance = U256::from_be_bytes(acc.balance.bytes);
    let nonce = acc.nonce;
    let code_hash = B256::from_slice(&acc.code_hash.bytes);

    AccountInfo {
        balance,
        nonce,
        code_hash,
        code: None, // code lazy-loaded on demand via code_by_hash
    }
}

// ---------------------------------------------------------------------------
//  Trait impls
// ---------------------------------------------------------------------------

impl DatabaseRef for GoDatabase {
    type Error = GoDBError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        unsafe {
            let mut out_info = FFIAccountInfo {
                balance: FFIU256 { bytes: [0u8; 32] },
                nonce: 0,
                code_hash: FFIHash { bytes: [0u8; 32] },
            };
            let ret = re_state_basic(
                self.handle,
                GoDatabase::address_to_ffi(address),
                &mut out_info as *mut _,
            );
            match ret {
                0 => Ok(Some(ffi_account_to_revm(&out_info))),
                1 => Ok(None), // not found (define convention)
                _ => Err(GoDBError("re_state_basic failed".into())),
            }
        }
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<revm::state::Bytecode, Self::Error> {
        unsafe {
            let mut ptr: *mut u8 = ptr::null_mut();
            let mut len: u32 = 0;
            let ret = re_state_code(
                self.handle,
                GoDatabase::hash_to_ffi(code_hash),
                &mut ptr as *mut _,
                &mut len as *mut _,
            );
            if ret == 1 {
                // not found; return empty bytecode
                return Ok(Bytecode::new());
            }
            if ret != 0 {
                return Err(GoDBError("re_state_code failed".into()));
            }
            if len == 0 || ptr.is_null() {
                return Ok(Bytecode::new());
            }
            let slice = std::slice::from_raw_parts(ptr, len as usize);
            let bytes = Bytes::copy_from_slice(slice);
            free(ptr as *mut c_void); // free C allocation
            Ok(Bytecode::new_raw(bytes))
        }
    }

    fn storage_ref(
        &self,
        address: Address,
        index: StorageKey,
    ) -> Result<StorageValue, Self::Error> {
        unsafe {
            let mut out = FFIU256 { bytes: [0u8; 32] };
            let ret = re_state_storage(
                self.handle,
                GoDatabase::address_to_ffi(address),
                GoDatabase::u256_to_ffi_hash(index),
                &mut out as *mut _,
            );
            if ret != 0 {
                return Err(GoDBError("re_state_storage failed".into()));
            }
            Ok(Self::ffi_u256_to_u256(out))
        }
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        unsafe {
            let mut out = FFIHash { bytes: [0u8; 32] };
            let ret = re_state_block_hash(self.handle, number, &mut out as *mut _);
            if ret != 0 {
                return Err(GoDBError("re_state_block_hash failed".into()));
            }
            Ok(GoDatabase::ffi_hash_to_b256(out))
        }
    }
}

impl Database for GoDatabase {
    type Error = GoDBError;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        self.basic_ref(address)
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.code_by_hash_ref(code_hash)
    }

    fn storage(
        &mut self,
        address: Address,
        index: StorageKey,
    ) -> Result<StorageValue, Self::Error> {
        self.storage_ref(address, index)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        self.block_hash_ref(number)
    }
}

impl DatabaseCommit for GoDatabase {
    fn commit(&mut self, changes: HashMap<Address, Account>) {
        println!("[Rust] GoDatabase.commit invoked, {} account(s)", changes.len());
        for (addr, account) in changes {
            // Debug print
            println!(
                "[Rust] COMMIT addr=0x{:x} nonce={} balance={:#x}",
                addr,
                account.info.nonce,
                account.info.balance
            );
            // commit basic
            let ffi_addr = GoDatabase::address_to_ffi(addr);
            let info = FFIAccountInfo {
                balance: GoDatabase::u256_to_ffi_u256(account.info.balance),
                nonce: account.info.nonce,
                code_hash: GoDatabase::hash_to_ffi(account.info.code_hash),
            };
            unsafe { re_state_set_basic(self.handle, ffi_addr, info); }

            // storage
            for (slot, value) in account.changed_storage_slots() {
                println!(
                    "[Rust] COMMIT_STORAGE addr=0x{:x} slot={:#x} value={:#x}",
                    addr,
                    slot,
                    value.present_value()
                );
                let ffi_slot = GoDatabase::u256_to_ffi_hash(*slot);
                let ffi_val = GoDatabase::u256_to_ffi_u256(value.present_value());
                unsafe { re_state_set_storage(self.handle, ffi_addr, ffi_slot, ffi_val); }
            }
        }
    }
}

// ---------------------------------------------------------------------------
//  Unit tests with mocked FFI callbacks
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    static CALLS_BASIC: AtomicUsize = AtomicUsize::new(0);

    // --- Mock implementations ---
    #[no_mangle]
    extern "C" fn re_state_basic(
        _handle: usize,
        _addr: FFIAddress,
        out_info: *mut FFIAccountInfo,
    ) -> i32 {
        unsafe {
            let info = FFIAccountInfo {
                balance: FFIU256 { bytes: [0u8; 32] },
                nonce: 42,
                code_hash: FFIHash { bytes: [0u8; 32] },
            };
            *out_info = info;
        }
        #[cfg(test)]
        {
            TEST_LAST_HANDLE.store(_handle, Ordering::SeqCst);
        }
        CALLS_BASIC.fetch_add(1, Ordering::SeqCst);
        0
    }

    #[no_mangle]
    extern "C" fn re_state_storage(
        _handle: usize,
        _addr: FFIAddress,
        _slot: FFIHash,
        out_val: *mut FFIU256,
    ) -> i32 {
        unsafe {
            *out_val = FFIU256 { bytes: [1u8; 32] };
        }
        0
    }

    #[no_mangle]
    extern "C" fn re_state_block_hash(
        _handle: usize,
        _number: u64,
        out_hash: *mut FFIHash,
    ) -> i32 {
        unsafe {
            *out_hash = FFIHash { bytes: [2u8; 32] };
        }
        0
    }

    #[no_mangle]
    extern "C" fn re_state_code(
        _handle: usize,
        _code_hash: FFIHash,
        out_ptr: *mut *mut u8,
        out_len: *mut u32,
    ) -> i32 {
        let data = vec![0xde, 0xad, 0xbe, 0xef];
        unsafe {
            let cbuf = libc::malloc(data.len()) as *mut u8;
            ptr::copy_nonoverlapping(data.as_ptr(), cbuf, data.len());
            *out_ptr = cbuf;
            *out_len = data.len() as u32;
        }
        0
    }

    #[test]
    fn test_basic() {
        let db = GoDatabase::new(1);
        let info = db
            .basic_ref(Address::ZERO)
            .expect("basic success")
            .expect("some");
        assert_eq!(info.nonce, 42);
        assert_eq!(CALLS_BASIC.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_storage() {
        let db = GoDatabase::new(1);
        let val = db
            .storage_ref(Address::ZERO, U256::ZERO)
            .expect("storage success");
        assert_eq!(val, U256::from_be_bytes([1u8; 32]));
    }

    #[test]
    fn test_code() {
        let db = GoDatabase::new(1);
        let bc = db
            .code_by_hash_ref(B256::ZERO)
            .expect("code");
        assert!(bc.bytes_slice().starts_with(&[0xde, 0xad, 0xbe, 0xef]));
    }
} 