//! FFI bindings for REVM (Rust Ethereum Virtual Machine)
//! 
//! This crate provides C-compatible FFI bindings for REVM, allowing other languages
//! like Go to interact with REVM through CGO.
//! 
//! # Safety
//! 
//! All FFI functions are marked as `unsafe` and require careful handling of memory
//! and pointer lifetimes. Callers must ensure proper cleanup of allocated resources.

#![cfg_attr(not(test), warn(unused_crate_dependencies))]

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::ptr;

use revm::{
    context::{CfgEnv, Context},
    // database::CacheDB,
    database_interface::EmptyDB,
    handler::MainnetEvm,
    database_interface::DatabaseCommit,
    primitives::hardfork::SpecId,
    ExecuteCommitEvm, ExecuteEvm, MainBuilder,
};

use revm::context_interface::context::ContextTr;
use revm::context_interface::journaled_state::JournalTr;
use revm::database_interface::Database;

// Additional primitives needed by generic helpers
use revm::primitives::{TxKind, U256, Bytes};
use std::slice;
use anyhow::Result;
use revm::handler::EvmTr;

mod types;
mod utils;
mod statedb_types;
mod go_db;

pub use types::*;
pub use utils::*;
pub use statedb_types::*;
pub use go_db::*;

// -----------------------------------------------------------------------------
//  Silence noisy stdout/stderr diagnostics unless `revm_verbose` feature
//  is explicitly enabled. We override the standard printing macros *for this
//  crate only*; downstream crates and dependencies are unaffected.
// -----------------------------------------------------------------------------

#[cfg(not(feature = "revm_verbose"))]
#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {};
}

#[cfg(not(feature = "revm_verbose"))]
#[macro_export]
macro_rules! eprintln {
    ($($arg:tt)*) => {};
}

#[allow(unused_macros)]
macro_rules! dbg_println {
    ($($arg:tt)*) => {{
        #[cfg(feature = "revm_verbose")]
        {
            println!($($arg)*);
        }
    }};
}

#[macro_export]
macro_rules! dbg_eprintln {
    ($($arg:tt)*) => {{
        ::std::eprintln!($($arg)*);
    }};
}

/// Initialize a new REVM instance
/// Returns a pointer to the EVM instance or null on failure
#[no_mangle]
pub unsafe extern "C" fn revm_new() -> *mut RevmInstance {
    let config = RevmConfigFFI::default();
    revm_new_with_config(&config)
}

/// Create a new REVM instance with a predefined chain preset
#[no_mangle]
pub extern "C" fn revm_new_with_preset(preset: ChainPreset) -> *mut RevmInstance {
    let config = match preset {
        ChainPreset::EthereumMainnet => RevmConfigFFI {
            chain_id: 1,
            spec_id: 19, // Prague
            ..Default::default()
        },
        ChainPreset::BSCMainnet => RevmConfigFFI {
            chain_id: 56,
            spec_id: 18, // Cancun (BSC is typically one hardfork behind)
            ..Default::default()
        },
        ChainPreset::BSCTestnet => RevmConfigFFI {
            chain_id: 97,
            spec_id: 18, // Cancun
            ..Default::default()
        },
        ChainPreset::Custom => RevmConfigFFI::default(), // Fallback to default
    };
    revm_new_with_config(&config)
}

/// Create a new REVM instance with custom configuration
#[no_mangle]
pub extern "C" fn revm_new_with_config(config: *const RevmConfigFFI) -> *mut RevmInstance {
    if config.is_null() {
        return ptr::null_mut();
    }
    
    let config = unsafe { &*config };
    
    // Convert spec_id to SpecId enum
    let spec_id = match config.spec_id {
        0 => SpecId::FRONTIER,
        1 => SpecId::FRONTIER_THAWING,
        2 => SpecId::HOMESTEAD,
        3 => SpecId::DAO_FORK,
        4 => SpecId::TANGERINE,
        5 => SpecId::SPURIOUS_DRAGON,
        6 => SpecId::BYZANTIUM,
        7 => SpecId::CONSTANTINOPLE,
        8 => SpecId::PETERSBURG,
        9 => SpecId::ISTANBUL,
        10 => SpecId::MUIR_GLACIER,
        11 => SpecId::BERLIN,
        12 => SpecId::LONDON,
        13 => SpecId::ARROW_GLACIER,
        14 => SpecId::GRAY_GLACIER,
        15 => SpecId::MERGE,
        16 => SpecId::SHANGHAI,
        17 => SpecId::CANCUN,
        18 => SpecId::CANCUN, // BSC uses Cancun-equivalent
        19 => SpecId::PRAGUE,
        20 => SpecId::OSAKA,
        _ => SpecId::PRAGUE, // Default to latest
    };
    
    // Create configuration environment
    let mut cfg_env = CfgEnv::new_with_spec(spec_id);
    cfg_env.chain_id = config.chain_id;
    cfg_env.disable_nonce_check = config.disable_nonce_check;
    
    // Set optional features if enabled
    #[cfg(feature = "optional_balance_check")]
    {
        cfg_env.disable_balance_check = config.disable_balance_check;
    }
    
    #[cfg(feature = "optional_block_gas_limit")]
    {
        cfg_env.disable_block_gas_limit = config.disable_block_gas_limit;
    }
    
    #[cfg(feature = "optional_no_base_fee")]
    {
        cfg_env.disable_base_fee = config.disable_base_fee;
    }
    
    // Disable-EIP-3607 (reject-sender-with-code) support is needed for almost
    // every unit-test, because many fixtures deliberately issue transactions
    // from pre-funded contract accounts. Always honour the Go-side flag,
    // irrespective of whether the upstream `optional_eip3607` cargo feature is
    // compiled in.
    cfg_env.disable_eip3607 = config.disable_eip3607;
    
    if config.max_code_size > 0 {
        cfg_env.limit_contract_code_size = Some(config.max_code_size as usize);
    }
    
    use revm::database::{CacheDB, EmptyDB};

    // Build an in-memory CacheDB wrapped around an EmptyDB (which always returns
    // default/zeroed accounts).  This is sufficient for unit-tests that
    // manually pre-fund accounts before execution.
    let cache_db: CacheDB<EmptyDB> = CacheDB::new(EmptyDB::default());

    // Construct the REVM execution context with the chosen spec and cfg env.
    let context = Context::new(cache_db, spec_id).with_cfg(cfg_env);
    let evm: MainnetEvm<_> = context.build_mainnet();

    // Wrap into our FFI struct and return a raw pointer for the caller.
    Box::into_raw(Box::new(RevmInstance {
        evm,
        last_error: None,
    }))
}

/// Free a REVM instance
#[no_mangle]
pub unsafe extern "C" fn revm_free(instance: *mut RevmInstance) {
    if !instance.is_null() {
        let _ = Box::from_raw(instance);
    }
}

/// Set transaction parameters
#[no_mangle]
pub unsafe extern "C" fn revm_set_tx(
    instance: *mut RevmInstance,
    caller: *const c_char,
    to: *const c_char,
    value: *const c_char,
    data: *const u8,
    data_len: c_uint,
    gas_limit: c_uint,
    gas_price: *const c_char,
    nonce: c_uint,
) -> c_int {
    if instance.is_null() {
        return -1;
    }
    
    let instance = &mut *instance;
    
    // Clear any previous error
    instance.last_error = None;
    
    match set_transaction_params(instance, caller, to, value, data, data_len, gas_limit, gas_price, nonce) {
        Ok(()) => 0,
        Err(e) => {
            instance.last_error = Some(e.to_string());
            -1
        }
    }
}

/// Execute a transaction (without committing state changes)
#[no_mangle]
pub unsafe extern "C" fn revm_execute(instance: *mut RevmInstance) -> *mut ExecutionResultFFI {
    if instance.is_null() {
        return ptr::null_mut();
    }
    
    let instance = &mut *instance;
    
    match instance.evm.replay() {
        Ok(result) => {
            // Debug: print gas refunded directly from REVM result
            match &result.result {
                revm::context_interface::result::ExecutionResult::Success { gas_refunded, gas_used, .. } => {
                    dbg_println!("[Rust] replay success: gas_used {}, refunded {}", gas_used, gas_refunded);
                },
                _ => {
                    dbg_println!("[Rust] replay result: {:?}", result.result);
                }
            }

            let ffi_result = convert_execution_result(result.result, None);
            Box::into_raw(Box::new(ffi_result))
        }
        Err(e) => {
            dbg_eprintln!("[Rust] evm.replay error: {}", e);
            instance.last_error = Some(format!("Execution failed: {:?}", e));
            ptr::null_mut()
        }
    }
}

/// Execute and commit a transaction
#[no_mangle]
pub unsafe extern "C" fn revm_execute_commit(instance: *mut RevmInstance) -> *mut ExecutionResultFFI {
    if instance.is_null() {
        return ptr::null_mut();
    }
    
    let instance = &mut *instance;
    
    match instance.evm.replay() {
        Ok(result_and_state) => {
            // Commit the diff into the inner CacheDB (simple EmptyDB backend).
            instance
                .evm
                .ctx()
                .journal()
                .db()
                .commit(result_and_state.state);

            Box::into_raw(Box::new(convert_execution_result(result_and_state.result, None)))
        }
        Err(e) => {
            dbg_eprintln!("[Rust] evm.replay error: {}", e);
            instance.last_error = Some(format!("Execution failed: {:?}", e));
            ptr::null_mut()
        }
    }
}

/// Deploy a contract
#[no_mangle]
pub unsafe extern "C" fn revm_deploy_contract(
    instance: *mut RevmInstance,
    deployer: *const c_char,
    bytecode: *const u8,
    bytecode_len: c_uint,
    gas_limit: c_uint,
) -> *mut DeploymentResultFFI {
    if instance.is_null() || bytecode.is_null() {
        return ptr::null_mut();
    }
    
    let instance = &mut *instance;
    
    match deploy_contract_impl(instance, deployer, bytecode, bytecode_len, gas_limit) {
        Ok(result) => Box::into_raw(Box::new(result)),
        Err(e) => {
            instance.last_error = Some(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Get account balance
#[no_mangle]
pub unsafe extern "C" fn revm_get_balance(
    instance: *mut RevmInstance,
    address: *const c_char,
) -> *mut c_char {
    if instance.is_null() || address.is_null() {
        return ptr::null_mut();
    }
    
    let instance = &mut *instance;
    
    match get_balance_impl(instance, address) {
        Ok(balance_str) => {
            match CString::new(balance_str) {
                Ok(c_str) => c_str.into_raw(),
                Err(_) => ptr::null_mut(),
            }
        }
        Err(e) => {
            instance.last_error = Some(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Set account balance
#[no_mangle]
pub unsafe extern "C" fn revm_set_balance(
    instance: *mut RevmInstance,
    address: *const c_char,
    balance: *const c_char,
) -> c_int {
    if instance.is_null() || address.is_null() || balance.is_null() {
        return -1;
    }
    
    let instance = &mut *instance;
    
    match set_balance_impl(instance, address, balance) {
        Ok(()) => 0,
        Err(e) => {
            instance.last_error = Some(e.to_string());
            -1
        }
    }
}

/// Get storage value
#[no_mangle]
pub unsafe extern "C" fn revm_get_storage(
    instance: *mut RevmInstance,
    address: *const c_char,
    slot: *const c_char,
) -> *mut c_char {
    if instance.is_null() || address.is_null() || slot.is_null() {
        return ptr::null_mut();
    }
    
    let instance = &mut *instance;
    
    match get_storage_impl(instance, address, slot) {
        Ok(value_str) => {
            match CString::new(value_str) {
                Ok(c_str) => c_str.into_raw(),
                Err(_) => ptr::null_mut(),
            }
        }
        Err(e) => {
            instance.last_error = Some(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Set storage value
#[no_mangle]
pub unsafe extern "C" fn revm_set_storage(
    instance: *mut RevmInstance,
    address: *const c_char,
    slot: *const c_char,
    value: *const c_char,
) -> c_int {
    if instance.is_null() || address.is_null() || slot.is_null() || value.is_null() {
        return -1;
    }
    
    let instance = &mut *instance;
    
    match set_storage_impl(instance, address, slot, value) {
        Ok(()) => 0,
        Err(e) => {
            instance.last_error = Some(e.to_string());
            -1
        }
    }
}

/// Get the last error message
#[no_mangle]
pub unsafe extern "C" fn revm_get_last_error(instance: *mut RevmInstance) -> *const c_char {
    if instance.is_null() {
        return ptr::null();
    }
    
    let instance = &*instance;
    
    match &instance.last_error {
        Some(error) => error.as_ptr() as *const c_char,
        None => ptr::null(),
    }
}

/// Free a C string allocated by this library
#[no_mangle]
pub unsafe extern "C" fn revm_free_string(s: *mut c_char) {
    if !s.is_null() {
        let _ = CString::from_raw(s);
    }
}

/// Free an execution result
#[no_mangle]
pub unsafe extern "C" fn revm_free_execution_result(result: *mut ExecutionResultFFI) {
    if !result.is_null() {
        let _ = Box::from_raw(result);
    }
}

/// Free a deployment result
#[no_mangle]
pub unsafe extern "C" fn revm_free_deployment_result(result: *mut DeploymentResultFFI) {
    if !result.is_null() {
        let _ = Box::from_raw(result);
    }
}

/// Get the chain ID of a REVM instance
#[no_mangle]
pub extern "C" fn revm_get_chain_id(instance: *const RevmInstance) -> u64 {
    if instance.is_null() {
        return 0;
    }
    
    let instance = unsafe { &*instance };
    instance.evm.ctx.cfg.chain_id
}

/// Get the spec ID of a REVM instance
#[no_mangle]
pub extern "C" fn revm_get_spec_id(instance: *const RevmInstance) -> u8 {
    if instance.is_null() {
        return 0;
    }
    
    let instance = unsafe { &*instance };
    instance.evm.ctx.cfg.spec as u8
}

/// Set account nonce
#[no_mangle]
pub unsafe extern "C" fn revm_set_nonce(
    instance: *mut RevmInstance,
    address: *const c_char,
    nonce: u64,
) -> c_int {
    if instance.is_null() || address.is_null() {
        return -1;
    }
    
    let instance = &mut *instance;
    
    match set_nonce_impl(instance, address, nonce) {
        Ok(()) => 0,
        Err(e) => {
            instance.last_error = Some(e.to_string());
            -1
        }
    }
}

/// Get account nonce
#[no_mangle]
pub unsafe extern "C" fn revm_get_nonce(
    instance: *mut RevmInstance,
    address: *const c_char,
) -> u64 {
    if instance.is_null() || address.is_null() {
        return 0;
    }
    
    let instance = &mut *instance;
    
    match get_nonce_impl(instance, address) {
        Ok(nonce) => nonce,
        Err(e) => {
            instance.last_error = Some(e.to_string());
            0
        }
    }
}

/// Transfer ETH between accounts
#[no_mangle]
pub unsafe extern "C" fn revm_transfer(
    instance: *mut RevmInstance,
    from: *const c_char,
    to: *const c_char,
    value: *const c_char,
    gas_limit: u64,
) -> *mut ExecutionResultFFI {
    if instance.is_null() || from.is_null() || to.is_null() || value.is_null() {
        return ptr::null_mut();
    }
    
    let instance = &mut *instance;
    
    match transfer_impl(instance, from, to, value, gas_limit) {
        Ok(result) => Box::into_raw(Box::new(result)),
        Err(e) => {
            instance.last_error = Some(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Call a contract
#[no_mangle]
pub unsafe extern "C" fn revm_call_contract(
    instance: *mut RevmInstance,
    from: *const c_char,
    to: *const c_char,
    data: *const u8,
    data_len: c_uint,
    value: *const c_char,
    gas_limit: u64,
) -> *mut ExecutionResultFFI {
    if instance.is_null() {
        return std::ptr::null_mut();
    }

    let instance_ref = &mut *instance;
    
    match call_contract_impl(instance_ref, from, to, data, data_len, value, gas_limit) {
        Ok(result) => Box::into_raw(Box::new(result)),
        Err(e) => {
            dbg_eprintln!("[Rust] call_contract error: {}", e);
            instance_ref.last_error = Some(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Call a contract function (view call - doesn't commit state)
#[no_mangle]
pub unsafe extern "C" fn revm_view_call_contract(
    instance: *mut RevmInstance,
    from: *const c_char,
    to: *const c_char,
    data: *const u8,
    data_len: c_uint,
    gas_limit: u64,
) -> *mut ExecutionResultFFI {
    if instance.is_null() {
        return std::ptr::null_mut();
    }

    let instance_ref = &mut *instance;
    
    match view_call_contract_impl(instance_ref, from, to, data, data_len, gas_limit) {
        Ok(result) => Box::into_raw(Box::new(result)),
        Err(e) => {
            dbg_eprintln!("[Rust] view_call_contract error: {}", e);
            instance_ref.last_error = Some(e.to_string());
            std::ptr::null_mut()
        }
    }
}

// Two-layer cache: the outer layer records writes for the current snapshot
// while the inner layer keeps the block-wide shared cache populated via
// prefetch.  `CacheDB::nest()` yields exactly this type.
type InnerGoDB = revm::database::CacheDB<GoDatabase>;
type NestedGoDB = revm::database::CacheDB<InnerGoDB>;

pub struct RevmInstanceStateDB {
    pub evm: MainnetEvm<
        revm::Context<
            revm::context::BlockEnv,
            revm::context::TxEnv,
            revm::context::CfgEnv,
            NestedGoDB,
            revm::Journal<NestedGoDB>,
            (),
        >,
    >,
    pub last_error: Option<String>,
}

/// Create a new REVM instance that sources all state via the given external
/// database handle (`handle`).  The Go side is expected to expose the four
/// `re_state_*` callbacks so that `GoDatabase` can service REVM look-ups.
#[no_mangle]
pub extern "C" fn revm_new_with_statedb(
    handle: usize,
    config: *const RevmConfigFFI,
) -> *mut RevmInstanceStateDB {
    // Obtain configuration (by value) – fallback to defaults if caller passed NULL.
    let cfg_val: RevmConfigFFI = if config.is_null() {
        RevmConfigFFI::default()
    } else {
        unsafe { std::ptr::read(config) }
    };

    use revm::primitives::hardfork::SpecId;
    // Map spec_id (u8) to the enum expected by REVM.
    let spec_id = match cfg_val.spec_id {
        0  => SpecId::FRONTIER,
        2  => SpecId::HOMESTEAD,
        4  => SpecId::TANGERINE,
        5  => SpecId::SPURIOUS_DRAGON,
        6  => SpecId::BYZANTIUM,
        7  => SpecId::CONSTANTINOPLE,
        8  => SpecId::PETERSBURG,
        9  => SpecId::ISTANBUL,
        11 => SpecId::BERLIN,
        12 => SpecId::LONDON,
        13 => SpecId::ARROW_GLACIER,
        14 => SpecId::GRAY_GLACIER,
        16 => SpecId::SHANGHAI,
        17 => SpecId::CANCUN,
        19 => SpecId::PRAGUE,
        20 => SpecId::OSAKA,
        _ => SpecId::PRAGUE,
    };

    // Build configuration environment.
    let mut cfg_env = CfgEnv::new_with_spec(spec_id);
    cfg_env.chain_id = cfg_val.chain_id;
    cfg_env.disable_nonce_check = cfg_val.disable_nonce_check;

    #[cfg(feature = "optional_balance_check")]
    {
        cfg_env.disable_balance_check = cfg_val.disable_balance_check;
    }
    #[cfg(feature = "optional_block_gas_limit")]
    {
        cfg_env.disable_block_gas_limit = cfg_val.disable_block_gas_limit;
    }
    #[cfg(feature = "optional_no_base_fee")]
    {
        cfg_env.disable_base_fee = cfg_val.disable_base_fee;
    }

    // Disable-EIP-3607 (reject-sender-with-code) support is needed for almost
    // every unit-test, because many fixtures deliberately issue transactions
    // from pre-funded contract accounts. Always honour the Go-side flag,
    // irrespective of whether the upstream `optional_eip3607` cargo feature is
    // compiled in.
    cfg_env.disable_eip3607 = cfg_val.disable_eip3607;

    if cfg_val.max_code_size > 0 {
        cfg_env.limit_contract_code_size = Some(cfg_val.max_code_size as usize);
    }

    // Build two-layer cache: GoDatabase → CacheDB (block) → nested CacheDB (tx snapshot).
    let external_db = GoDatabase::new(handle);
    let inner_db: InnerGoDB = revm::database::CacheDB::new(external_db);
    let nested_db: NestedGoDB = inner_db.nest();

    let context = Context::new(nested_db, spec_id).with_cfg(cfg_env);
    let evm = context.build_mainnet();

    Box::into_raw(Box::new(RevmInstanceStateDB {
        evm,
        last_error: None,
    }))
}

/// Free a `RevmInstanceStateDB` instance
#[no_mangle]
pub unsafe extern "C" fn revm_free_statedb_instance(instance: *mut RevmInstanceStateDB) {
    if !instance.is_null() {
        let _ = Box::from_raw(instance);
    }
}

/// Call a contract via StateDB-backed instance
#[no_mangle]
pub unsafe extern "C" fn revm_call_contract_statedb(
    instance: *mut RevmInstanceStateDB,
    from: *const c_char,
    to: *const c_char,
    data: *const u8,
    data_len: c_uint,
    value: *const c_char,
    gas_limit: u64,
) -> *mut ExecutionResultFFI {
    use crate::utils::{c_str_to_string, hex_to_address, hex_to_u256, convert_execution_result};
    use std::io::Write;

    if instance.is_null() {
        return std::ptr::null_mut();
    }

    let inst = &mut *instance;
    let evm = &mut inst.evm;

    dbg_println!("[Rust] revm_call_contract_statedb invoked, instance={:p}", instance);
    std::io::stdout().flush().ok();

    // Decode `from` (must be present)
    let from_addr = match c_str_to_string(from).and_then(|s| hex_to_address(&s)) {
        Ok(a) => a,
        Err(e) => {
            inst.last_error = Some(e.to_string());
            return std::ptr::null_mut();
        }
    };

    // Decode `to` (optional – empty means contract creation)
    let to_addr_opt: Option<revm::primitives::Address> = if to.is_null() {
        None
    } else {
        match c_str_to_string(to) {
            Ok(s) if s.is_empty() => None,
            Ok(s) => match hex_to_address(&s) {
                Ok(addr) => Some(addr),
                Err(e) => { inst.last_error = Some(e.to_string()); return std::ptr::null_mut(); }
            },
            Err(_) => None, // treat invalid c-string as None
        }
    };

    let value_u256 = if value.is_null() {
        U256::ZERO
    } else {
        match c_str_to_string(value).and_then(|s| hex_to_u256(&s)) {
            Ok(v) => v,
            Err(e) => {
                inst.last_error = Some(e.to_string());
                return std::ptr::null_mut();
            }
        }
    };

    let call_data = if data.is_null() || data_len == 0 {
        Bytes::new()
    } else {
        let slice = std::slice::from_raw_parts(data, data_len as usize);
        Bytes::copy_from_slice(slice)
    };

    // Chain ID from cfg env
    let chain_id = evm.ctx().cfg.chain_id;

    // Determine nonce & balance for debug
    let (current_nonce, from_balance) = match evm.ctx().journal().db().basic(from_addr) {
        Ok(opt) => {
            if let Some(acc) = opt {
                dbg_println!("[Rust] DB basic nonce={} balance={}", acc.nonce, acc.balance);
                (acc.nonce, acc.balance)
            } else {
                (0, U256::ZERO)
            }
        }
        Err(e) => {
            inst.last_error = Some(e.to_string());
            return std::ptr::null_mut();
        }
    };

    // Populate TxEnv through the new safe modifier helpers
    evm.ctx().modify_tx(|tx| {
        tx.caller = from_addr;
        tx.kind = match to_addr_opt {
            Some(addr) => TxKind::Call(addr),
            None => TxKind::Create,
        };
        tx.data = call_data;
        tx.value = value_u256;
        tx.gas_limit = gas_limit;
        tx.gas_price = 1_000_000_000u128; // 1 gwei
    });

    match evm.replay() {
        Ok(res) => Box::into_raw(Box::new(convert_execution_result(res.result))),
        Err(e) => {
            dbg_eprintln!("[Rust] evm.replay error: {}", e);
            inst.last_error = Some(e.to_string());
            std::ptr::null_mut()
        }
    }
}

/// Call a contract via StateDB-backed instance with commit
#[no_mangle]
pub unsafe extern "C" fn revm_call_contract_statedb_commit(
    instance: *mut RevmInstanceStateDB,
    from: *const c_char,
    to: *const c_char,
    data: *const u8,
    data_len: c_uint,
    value: *const c_char,
    gas_limit: u64,
) -> *mut ExecutionResultFFI {
    if instance.is_null() {
        return ptr::null_mut();
    }
    let instance = &mut *instance;
    let from_addr = parse_address(from);
    let to_addr = parse_address(to);
    let value_u256 = parse_u256(value);
    let data_slice = slice::from_raw_parts(data, data_len as usize);

    let mut evm = &mut instance.evm;

    // -----------------------------------------------------------------
    // Determine the correct sender nonce so that REVM state transition
    // validates and subsequently bumps it. We fetch the current on-chain
    // account info from the backing database. If the account does not yet
    // exist we start from zero – this matches legacy Go-EVM behaviour.
    // -----------------------------------------------------------------
    let current_nonce: u64 = match evm.ctx().journal().db().basic(from_addr) {
        Ok(opt) => opt.map(|acc| acc.nonce).unwrap_or(0),
        Err(e) => {
            instance.last_error = Some(e.to_string());
            return ptr::null_mut();
        }
    };

    // Populate TxEnv through the new safe modifier helpers, including the
    // discovered nonce so that the handler increments it on success.
    evm.ctx().modify_tx(|tx| {
        tx.caller = from_addr;
        tx.kind = if to_addr.is_zero() {
            TxKind::Create
        } else {
            TxKind::Call(to_addr)
        };
        tx.data = Bytes::from(data_slice.to_vec());
        tx.value = value_u256;
        tx.gas_limit = gas_limit;
        tx.gas_price = 1_000_000_000u128; // 1 gwei
        tx.nonce = current_nonce;
    });

    let exec_res = match evm.replay() {
        Ok(mut result_and_state) => {
            // Commit the precise diff returned by REVM into both cache layers
            // and directly into Go's StateDB.
            {
                // Ensure the caller's nonce increment is included even if the
                // underlying handler fails to mark it as a diff. We bump it
                // by one relative to the nonce we observed prior to execution.
                use revm::state::Account;
                use revm::primitives::HashMap as RevHashMap;

                let mut state_diff: RevHashMap<revm::primitives::Address, Account> = result_and_state.state.clone();

                state_diff.entry(from_addr).and_modify(|acc| {
                    acc.info.nonce = current_nonce.saturating_add(1);
                }).or_insert_with(|| {
                    let mut acc = Account::default();
                    acc.info.nonce = current_nonce.saturating_add(1);
                    acc.status = revm::state::AccountStatus::Touched;
                    acc
                });

                let parent_db: &mut NestedGoDB = evm.ctx().journal().db();
                parent_db.commit(state_diff.clone());            // outer-cache merge
                parent_db.db.db.commit(state_diff.clone());       // Go StateDB
            }

            // Keep nested caches consistent.
            propagate_cached_changes(evm.ctx().journal().db());

            Box::into_raw(Box::new(convert_execution_result(result_and_state.result, None)))
        }
        Err(e) => {
            dbg_eprintln!("[Rust] evm.replay error: {}", e);
            instance.last_error = Some(format!("Execution failed: {:?}", e));
            ptr::null_mut()
        }
    };
    exec_res
}

/// Update the spec-id (hard-fork rules) of an existing REVM instance that is
/// backed by a Go StateDB. This lets the Go side switch between Frontier/
/// London/Prague … rules without having to recreate the whole instance.
/// The mapping of the numeric `id` matches the one used in `revm_new_with_statedb`.
#[no_mangle]
pub extern "C" fn revm_set_spec_id(instance: *mut RevmInstanceStateDB, id: u8) {
    if instance.is_null() {
        return;
    }
    // Safety: caller guarantees the pointer is valid for the lifetime of the call.
    let inst = unsafe { &mut *instance };

    use revm::primitives::hardfork::SpecId;

    // Convert numeric ID (as used by Go layer) to REVM SpecId.
    let spec = match id {
        0  => SpecId::FRONTIER,
        2  => SpecId::HOMESTEAD,
        4  => SpecId::TANGERINE,
        5  => SpecId::SPURIOUS_DRAGON,
        6  => SpecId::BYZANTIUM,
        7  => SpecId::CONSTANTINOPLE,
        8  => SpecId::PETERSBURG,
        9  => SpecId::ISTANBUL,
        11 => SpecId::BERLIN,
        12 => SpecId::LONDON,
        13 => SpecId::ARROW_GLACIER,
        14 => SpecId::GRAY_GLACIER,
        16 => SpecId::SHANGHAI,
        17 => SpecId::CANCUN,
        19 => SpecId::PRAGUE,
        20 => SpecId::OSAKA,
        _  => SpecId::PRAGUE,
    };

    inst.evm.ctx.cfg.spec = spec;

    dbg_println!("[revm_set_spec_id] id={} spec={:?}", id, spec);
}

/// Retrieve the last error string for a StateDB-backed instance.
#[no_mangle]
pub extern "C" fn revm_last_error_statedb(instance: *mut RevmInstanceStateDB) -> *const c_char {
    if instance.is_null() {
        return std::ptr::null();
    }
    let inst = unsafe { &mut *instance };
    if let Some(ref err) = inst.last_error {
        let cstr = CString::new(err.clone()).unwrap();
        let ptr = cstr.as_ptr();
        std::mem::forget(cstr); // leak to C caller; they must free
        ptr
    } else {
        std::ptr::null()
    }
}

/// Set account code bytes
#[no_mangle]
pub unsafe extern "C" fn revm_set_code(
    instance: *mut RevmInstance,
    address: *const c_char,
    code: *const u8,
    code_len: c_uint,
) -> c_int {
    if instance.is_null() {
        return -1;
    }

    let instance = &mut *instance;
    instance.last_error = None;

    match crate::utils::set_code_impl(instance, address, code, code_len) {
        Ok(()) => 0,
        Err(e) => {
            instance.last_error = Some(e.to_string());
            -1
        }
    }
}

// ---------------------------------------------------------------------------
//  Tests – ensure the constructor works and produces a usable instance.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod statedb_constructor_tests {
    use super::*;
    use revm::handler::EvmTr;
    use revm::primitives::Address;
    use super::go_db::TEST_LAST_HANDLE;

    #[test]
    fn test_revm_new_with_statedb_returns_instance() {
        // Dummy handle
        TEST_LAST_HANDLE.store(12345, std::sync::atomic::Ordering::SeqCst);
        let config = RevmConfigFFI::default();
        let instance = revm_new_with_statedb(0, &config);
        assert!(!instance.is_null());
        unsafe { revm_free_statedb_instance(instance) };
        assert_eq!(TEST_LAST_HANDLE.load(std::sync::atomic::Ordering::SeqCst), 12345);
    }
}

fn convert_execution_result(
    result: revm::context_interface::result::ExecutionResult,
    tx_hash: Option<revm::primitives::B256>,
) -> ExecutionResultFFI {
    use revm::context_interface::result::{ExecutionResult, Output};
    match result {
        ExecutionResult::Success {
            reason,
            gas_used,
            gas_refunded,
            logs,
            output,
        } => {
            let (output_data, output_len) = match &output {
                Output::Call(data) => (data.as_ptr(), data.len()),
                Output::Create(data, _) => (data.as_ptr(), data.len()),
            };
            let created_address = match output {
                Output::Create(_, Some(addr)) => {
                    let c_addr = CString::new(addr.to_string()).unwrap();
                    c_addr.into_raw()
                }
                _ => ptr::null_mut(),
            };

            let logs_count = logs.len();
            let ffi_logs: Vec<LogFFI> = logs.into_iter().map(LogFFI::from_revm_log).collect();
            let logs_ptr = if logs_count > 0 {
                Box::into_raw(ffi_logs.into_boxed_slice()) as *mut c_void
            } else {
                ptr::null_mut()
            };

            ExecutionResultFFI {
                success: 1,
                gas_used: gas_used as c_uint,
                gas_refunded: gas_refunded as c_uint,
                output_data: output_data as *mut u8,
                output_len: output_len as c_uint,
                logs_count: logs_count as c_uint,
                logs: logs_ptr,
                created_address,
                tx_hash: ptr::null_mut(),
            }
        }
        ExecutionResult::Revert { gas_used, output } => {
            ExecutionResultFFI {
                success: 0,
                gas_used: gas_used as c_uint,
                gas_refunded: 0,
                output_data: output.as_ptr() as *mut u8,
                output_len: output.len() as c_uint,
                logs_count: 0,
                logs: ptr::null_mut(),
                created_address: ptr::null_mut(),
                tx_hash: ptr::null_mut(),
            }
        }
        ExecutionResult::Halt { reason, gas_used } => {
            ExecutionResultFFI {
                success: 0,
                gas_used: gas_used as c_uint,
                gas_refunded: 0,
                output_data: ptr::null_mut(),
                output_len: 0,
                logs_count: 0,
                logs: ptr::null_mut(),
                created_address: ptr::null_mut(),
                tx_hash: ptr::null_mut(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
//  Internal helpers for older wrapper paths -> new API (REVM v24)
// ---------------------------------------------------------------------------

unsafe fn parse_address(ptr: *const c_char) -> revm::primitives::Address {
    if ptr.is_null() {
        return revm::primitives::Address::ZERO;
    }
    let s = c_str_to_string(ptr).unwrap_or_else(|_| "0x00".to_string());
    hex_to_address(&s).unwrap_or(revm::primitives::Address::ZERO)
}

unsafe fn parse_u256(ptr: *const c_char) -> U256 {
    if ptr.is_null() {
        return U256::ZERO;
    }
    let s = c_str_to_string(ptr).unwrap_or_else(|_| "0x0".to_string());
    hex_to_u256(&s).unwrap_or(U256::ZERO)
}

// ---------------- batch prefetch (stub impl) ----------------

use crate::statedb_types::{FFIAddress, FFIHash};

#[repr(C)]
pub struct FFIBatchKey {
    address: FFIAddress,
    slot: FFIHash,
}

/// Best-effort batch prefetch.  Walks the provided (address,slot) list once,
/// touches each account / storage entry on REVM's database, and thereby
/// populates the instance-local `CacheDB`.  Subsequent transaction execution
/// can then serve these look-ups from memory without crossing the CGO
/// boundary.
///
/// * A zeroed `slot` means "account-only" prefetch (no storage).
/// * Duplicate keys are automatically de-duplicated inside this helper.
/// * Unknown accounts / slots are silently ignored – we only prime the cache.
///
/// Safety: called from Go; must guard against NULL and out-of-bounds inputs –
/// we return immediately on invalid pointers.
#[no_mangle]
pub extern "C" fn revm_prefetch_batch(
    inst: *mut RevmInstanceStateDB,
    keys: *const FFIBatchKey,
    count: libc::size_t,
) {
    use std::collections::HashSet;
    use revm::primitives::{Address, U256};

    if inst.is_null() || keys.is_null() || count == 0 {
        return;
    }

    // SAFETY: pointers checked for NULL above; slice bounds derive from count.
    let slice = unsafe { core::slice::from_raw_parts(keys, count as usize) };
    let evm = unsafe { &mut (*inst).evm };

    // De-duplicate accounts and storage keys to avoid redundant DB calls.
    let mut acc_set: HashSet<Address> = HashSet::with_capacity(slice.len());
    let mut stor_set: HashSet<(Address, U256)> = HashSet::new();

    for k in slice {
        let addr = Address::from_slice(&k.address.bytes);
        acc_set.insert(addr);

        // All-zero slot means "account only".
        if k.slot.bytes.iter().any(|&b| b != 0) {
            let slot_u256 = U256::from_be_bytes(k.slot.bytes);
            stor_set.insert((addr, slot_u256));
        }
    }

    // Prime CacheDB by issuing `basic` / `storage` calls. Ignore errors – this
    // is only a performance hint; any miss will be fetched lazily later.
    for addr in acc_set {
        let _ = evm.ctx().journal().db().basic(addr);
    }

    for (addr, slot) in stor_set {
        let _ = evm.ctx().journal().db().storage(addr, slot);
    }
}

/// Create a lightweight snapshot (clone) of the given `RevmInstanceStateDB`.
///
/// The returned pointer owns an independent `RevmInstanceStateDB` value whose
/// internal EVM is *cloned* from the parent.  All pointers remain valid as the
/// underlying database (`CacheDB<GoDatabase>`) implements `Clone` cheaply by
/// reference-copying the external handle and duplicating the in-memory cache.
///
/// NOTE: For Phase-1 plumbing this is a deep clone; subsequent phases will
/// switch to `CacheDB::nest()` to provide true copy-on-write snapshotting.
#[no_mangle]
pub unsafe extern "C" fn revm_snapshot_clone(
    parent: *mut RevmInstanceStateDB,
) -> *mut RevmInstanceStateDB {
    if parent.is_null() {
        return std::ptr::null_mut();
    }

    let parent_mut = &mut *parent;

    // Parent DB layout: OuterCache<Block> (NestedGoDB) holding an Inner
    // CacheDB<Block> which itself wraps GoDatabase. For snapshot we want a
    // fresh outer layer over a *clone* of the shared inner cache so that all
    // prefetched data stays hot.
    let parent_db: &mut NestedGoDB = parent_mut.evm.journal().db();
    // Flatten parent to merge its outer cache into the shared inner cache so
    // prefetched accounts are visible in snapshots.
    let base_inner: InnerGoDB = parent_db.clone().flatten();
    let child_db: NestedGoDB = base_inner.nest();

    // Recreate context with the nested DB while cloning other env data.
    let child_ctx = parent_mut.evm.ctx().clone().with_db(child_db);
    let child_evm = child_ctx.build_mainnet();

    Box::into_raw(Box::new(RevmInstanceStateDB {
        evm: child_evm,
        last_error: None,
    }))
}

/// Commit the changes from a snapshot back into its parent instance and free
/// the snapshot.  Both pointers must be non-null and distinct. After a
/// successful commit the `child` pointer must not be used again (it is freed
/// internally).
#[no_mangle]
pub unsafe extern "C" fn revm_snapshot_commit(
    parent: *mut RevmInstanceStateDB,
    child: *mut RevmInstanceStateDB,
) {
    if parent.is_null() || child.is_null() || parent == child {
        return;
    }

    let parent_ref = &mut *parent;
    // Take ownership of the child so we can safely drop it later.
    let mut child_box = Box::from_raw(child);

    // Access parent's database now; we will access the child's DB later, after
    // we have extracted the journal diff to avoid overlapping mutable borrows.
    let parent_db: &mut NestedGoDB = (*parent_ref.evm).journal().db();

    // -------------------------------------------------------------------
    // 1. Extract the *exact* state diff recorded by the child's journal.
    //    This captures every account/storage change, even those that do not
    //    leave a trace in CacheDB (e.g. SSTORE-to-zero, selfdestruct).
    // -------------------------------------------------------------------
    let state_diff = {
        let journal = child_box.evm.ctx().journal();
        journal.finalize().state
    };

    // -------------------------------------------------------------------
    // 2. Merge the diff into both layers:
    //    a) Parent outer cache – so subsequent REVM reads see the update.
    //    b) GoDatabase – so Go-StateDB (trie) is updated immediately.
    // -------------------------------------------------------------------
    parent_db.commit(state_diff.clone());        // outer-cache merge
    parent_db.db.db.commit(state_diff.clone());       // Go StateDB

    // -------------------------------------------------------------------
    // 3. Now fold the child's outer CacheDB into its inner cache and install
    //    that inner cache as the new shared base inside the parent.
    // -------------------------------------------------------------------
    // Now safe to access child's DB.
    let child_db: &mut NestedGoDB = (*child_box.evm).journal().db();

    use std::mem::replace;
    let moved_child: NestedGoDB = replace(child_db, parent_db.db.clone().nest());
    let flattened_inner: InnerGoDB = moved_child.flatten();
    parent_db.db = flattened_inner;

    // 4. (Optional) keep caches consistent for other layers.
    propagate_cached_changes(parent_db);

    // Child is automatically dropped here freeing memory.
}

/// Clear both outer (tx-snapshot) and inner (block-wide) CacheDB layers so subsequent
/// look-ups observe the authoritative Go StateDB. This should be invoked after the
/// pending journal has been flushed at the end of every transaction or whenever
/// external code mutates the underlying StateDB outside of REVM (e.g. miner reward
/// application in `engine.Finalize`).
#[no_mangle]
pub extern "C" fn revm_clear_caches_statedb(instance: *mut RevmInstanceStateDB) {
    use std::mem::take;

    if instance.is_null() {
        return;
    }

    // Safety: the caller guarantees the pointer is valid for the lifetime of the call.
    let inst = unsafe { &mut *instance };

    // Wipe both cache layers in-place. We intentionally keep the structs allocated –
    // only the collections storing cached objects are cleared.
    inst.evm.ctx().modify_db(|nested_db| {
        // Helper to reset a single Cache instance.
        fn clear_cache(cache: &mut revm::database::in_memory_db::Cache) {
            cache.accounts.clear();
            cache.contracts.clear();
            cache.logs.clear();
            cache.block_hashes.clear();
        }

        // Clear ONLY the outer (tx-snapshot) cache. The block-wide inner
        // cache is retained so that subsequent transactions can still serve
        // hot reads from RAM, avoiding CGO round-trips. It already contains
        // the freshly committed diffs, so it is safe to keep.
        clear_cache(&mut nested_db.cache);
    });
}

// Helper that walks over a NestedGoDB and forwards any touched accounts / storage
// to the Go StateDB via GoDatabase.commit. Used after both snapshot commits and
// direct replay_commit calls.
fn propagate_cached_changes(outer_db: &mut NestedGoDB) {
    use revm::{state::{Account, EvmStorageSlot, AccountStatus}, primitives::{HashMap as RevHashMap, StorageValue}};

    // Build iterator over both outer and inner cache layers so we capture
    // direct replay_commit (inner layer) as well as snapshot commits (outer layer).
    let total_accs = outer_db.cache.accounts.len() + outer_db.db.cache.accounts.len();
    if total_accs == 0 {
        return;
    }

    let mut changes: RevHashMap<revm::primitives::Address, Account> = RevHashMap::with_capacity(total_accs);

    for (addr, db_acc) in outer_db.cache.accounts.iter().chain(outer_db.db.cache.accounts.iter()) {
        let mut storage_map = RevHashMap::new();
        for (slot, val) in &db_acc.storage {
            eprintln!("[propagate]   slot={:#x} val={:#x}", slot, val);
            storage_map.insert(*slot, EvmStorageSlot {
                original_value: StorageValue::ZERO,
                present_value: *val,
                is_cold: false,
            });
        }

        eprintln!("[propagate] ACC 0x{:x} nonce={} bal={:#x} storage_slots={} ", addr, db_acc.info.nonce, db_acc.info.balance, db_acc.storage.len());

        let account = Account {
            info: db_acc.info.clone(),
            storage: storage_map,
            status: AccountStatus::Touched,
        };
        changes.insert(*addr, account);
    }

    if !changes.is_empty() {
        eprintln!("[Rust] propagate_cached_changes: pushing {} accounts", changes.len());
        outer_db.db.commit(changes);
    }

    eprintln!("[Rust] propagate_cached_changes end");
} 