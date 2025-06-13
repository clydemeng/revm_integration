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
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;

use revm::{
    context::{CfgEnv, Context},
    database::CacheDB,
    database_interface::EmptyDB,
    handler::MainnetEvm,
    primitives::hardfork::SpecId,
    ExecuteCommitEvm, ExecuteEvm, MainBuilder,
};

mod types;
mod utils;
mod statedb_types;
mod go_db;

pub use types::*;
pub use utils::*;
pub use statedb_types::*;
pub use go_db::*;

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
    
    if config.max_code_size > 0 {
        cfg_env.limit_contract_code_size = Some(config.max_code_size as usize);
    }
    
    // Create cache database and context with custom configuration
    let cache_db = CacheDB::<EmptyDB>::default();
    let context = Context::new(cache_db, spec_id)
        .with_cfg(cfg_env);
    
    let evm = context.build_mainnet();
    
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
            let ffi_result = convert_execution_result(result.result);
            Box::into_raw(Box::new(ffi_result))
        }
        Err(e) => {
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
    
    match instance.evm.replay_commit() {
        Ok(result) => {
            let ffi_result = convert_execution_result(result);
            Box::into_raw(Box::new(ffi_result))
        }
        Err(e) => {
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
            instance_ref.last_error = Some(e.to_string());
            std::ptr::null_mut()
        }
    }
} 