//! Utility functions for FFI operations

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_uint};
use std::slice;

use anyhow::{anyhow, Result};
use revm::{
    context_interface::{
        result::{ExecutionResult, HaltReason, Output},
        context::ContextTr,
        journaled_state::JournalTr,
    },
    handler::{EvmTr, ExecuteCommitEvm},
    primitives::{Address, Bytes, TxKind, U256},
    database_interface::Database,
    context::{CfgEnv, Context},
    database::{CacheDB, EmptyDB},
    state::AccountInfo,
    ExecuteEvm,
    handler::MainnetEvm,
};

use crate::types::{DeploymentResultFFI, ExecutionResultFFI, RevmInstance};

/// Convert a C string to a Rust string
pub unsafe fn c_str_to_string(c_str: *const c_char) -> Result<String> {
    if c_str.is_null() {
        return Err(anyhow!("Null pointer"));
    }
    
    let c_str = CStr::from_ptr(c_str);
    c_str.to_str()
        .map(|s| s.to_string())
        .map_err(|e| anyhow!("Invalid UTF-8: {}", e))
}

/// Convert a hex string to Address
pub fn hex_to_address(hex_str: &str) -> Result<Address> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    if hex_str.len() != 40 {
        return Err(anyhow!("Invalid address length"));
    }
    
    let bytes = hex::decode(hex_str)?;
    if bytes.len() != 20 {
        return Err(anyhow!("Address must be 20 bytes"));
    }
    
    let mut addr_bytes = [0u8; 20];
    addr_bytes.copy_from_slice(&bytes);
    Ok(Address::from(addr_bytes))
}

/// Convert a hex string to U256
pub fn hex_to_u256(hex_str: &str) -> Result<U256> {
    let hex_str = hex_str.strip_prefix("0x").unwrap_or(hex_str);
    U256::from_str_radix(hex_str, 16).map_err(|e| anyhow!("Invalid U256: {}", e))
}

/// Convert U256 to hex string
pub fn u256_to_hex(value: U256) -> String {
    format!("0x{:x}", value)
}

/// Convert Address to hex string
pub fn address_to_hex(addr: Address) -> String {
    format!("0x{:x}", addr)
}

/// Convert bytes to hex string
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

/// Convert REVM execution result to FFI result
pub fn convert_execution_result(result: ExecutionResult<HaltReason>) -> ExecutionResultFFI {
    match result {
        ExecutionResult::Success { reason: _, gas_used, gas_refunded, output, logs } => {
            let (output_data, output_len) = match output {
                Output::Call(bytes) | Output::Create(bytes, _) => {
                    if bytes.is_empty() {
                        (std::ptr::null_mut(), 0)
                    } else {
                        let len = bytes.len();
                        let data = bytes.to_vec().into_boxed_slice();
                        (Box::into_raw(data) as *mut u8, len as c_uint)
                    }
                }
            };

            ExecutionResultFFI {
                success: 1,
                gas_used: gas_used.try_into().unwrap_or(u32::MAX),
                gas_refunded: gas_refunded.try_into().unwrap_or(u32::MAX),
                output_data,
                output_len,
                logs_count: logs.len() as c_uint,
                logs: {
                    if logs.is_empty() {
                        std::ptr::null_mut()
                    } else {
                        let mut ffi_logs: Vec<crate::types::LogFFI> = Vec::with_capacity(logs.len());
                        for l in logs {
                            ffi_logs.push(crate::types::LogFFI::from_revm_log(l));
                        }
                        let boxed = ffi_logs.into_boxed_slice();
                        Box::into_raw(boxed) as *mut crate::types::LogFFI
                    }
                },
                created_address: std::ptr::null_mut(),
            }
        }
        ExecutionResult::Revert { gas_used, output } => {
            let (output_data, output_len) = if output.is_empty() {
                (std::ptr::null_mut(), 0)
            } else {
                let len = output.len();
                let data = output.to_vec().into_boxed_slice();
                (Box::into_raw(data) as *mut u8, len as c_uint)
            };

            ExecutionResultFFI {
                success: 0,
                gas_used: gas_used.try_into().unwrap_or(u32::MAX),
                gas_refunded: 0,
                output_data,
                output_len,
                logs_count: 0,
                logs: std::ptr::null_mut(),
                created_address: std::ptr::null_mut(),
            }
        }
        ExecutionResult::Halt { reason: _, gas_used } => {
            ExecutionResultFFI {
                success: -1,
                gas_used: gas_used.try_into().unwrap_or(u32::MAX),
                gas_refunded: 0,
                output_data: std::ptr::null_mut(),
                output_len: 0,
                logs_count: 0,
                logs: std::ptr::null_mut(),
                created_address: std::ptr::null_mut(),
            }
        }
    }
}

/// Set transaction parameters
pub unsafe fn set_transaction_params(
    instance: &mut RevmInstance,
    caller: *const c_char,
    to: *const c_char,
    value: *const c_char,
    data: *const u8,
    data_len: c_uint,
    gas_limit: c_uint,
    gas_price: *const c_char,
    nonce: c_uint,
) -> Result<()> {
    let caller_addr = hex_to_address(&c_str_to_string(caller)?)?;
    
    let kind = if to.is_null() {
        TxKind::Create
    } else {
        let to_addr = hex_to_address(&c_str_to_string(to)?)?;
        TxKind::Call(to_addr)
    };
    
    let value = if value.is_null() {
        U256::ZERO
    } else {
        hex_to_u256(&c_str_to_string(value)?)?
    };
    
    let data = if data.is_null() || data_len == 0 {
        Bytes::new()
    } else {
        let slice = slice::from_raw_parts(data, data_len as usize);
        Bytes::copy_from_slice(slice)
    };
    
    let gas_price = if gas_price.is_null() {
        1_000_000_000u128 // 1 gwei default
    } else {
        hex_to_u256(&c_str_to_string(gas_price)?)?.try_into().unwrap_or(1_000_000_000u128)
    };

    instance.evm.ctx().modify_tx(|tx| {
        tx.caller = caller_addr;
        tx.kind = kind;
        tx.value = value;
        tx.data = data;
        tx.gas_limit = gas_limit as u64;
        tx.gas_price = gas_price;
        tx.nonce = nonce as u64;
    });

    Ok(())
}

/// Deploy a contract
pub unsafe fn deploy_contract_impl(
    instance: &mut RevmInstance,
    deployer: *const c_char,
    bytecode: *const u8,
    bytecode_len: c_uint,
    gas_limit: c_uint,
) -> Result<DeploymentResultFFI> {
    let deployer_addr = hex_to_address(&c_str_to_string(deployer)?)?;
    let bytecode_slice = slice::from_raw_parts(bytecode, bytecode_len as usize);
    let bytecode_bytes = Bytes::copy_from_slice(bytecode_slice);

    // Get the chain ID from the context
    let chain_id = instance.evm.ctx.cfg.chain_id;
    
    // Get the current nonce for the deployer
    let current_nonce = {
        let account = instance.evm.ctx().journal().db().basic(deployer_addr)?;
        match account {
            Some(acc) => acc.nonce,
            None => 0,
        }
    };

    instance.evm.ctx().modify_tx(|tx| {
        tx.caller = deployer_addr;
        tx.kind = TxKind::Create;
        tx.data = bytecode_bytes;
        tx.gas_limit = gas_limit as u64;
        tx.gas_price = 1_000_000_000u128; // 1 gwei
        tx.nonce = current_nonce;
        tx.value = U256::ZERO;
        tx.chain_id = Some(chain_id);
    });

    let result = instance.evm.replay_commit()?;
    
    match result {
        ExecutionResult::Success { gas_used, output, .. } => {
            let contract_address = match output {
                Output::Create(_, Some(addr)) => {
                    let addr_str = address_to_hex(addr);
                    CString::new(addr_str)?.into_raw()
                }
                _ => std::ptr::null_mut(),
            };

            Ok(DeploymentResultFFI {
                success: 1,
                contract_address,
                gas_used: gas_used.try_into().unwrap_or(u32::MAX),
                gas_refunded: 0,
            })
        }
        _ => Ok(DeploymentResultFFI {
            success: 0,
            contract_address: std::ptr::null_mut(),
            gas_used: 0,
            gas_refunded: 0,
        }),
    }
}

/// Get account balance
pub unsafe fn get_balance_impl(
    instance: &mut RevmInstance,
    address: *const c_char,
) -> Result<String> {
    let addr = hex_to_address(&c_str_to_string(address)?)?;
    let account = instance.evm.ctx().journal().db().basic(addr)?;
    
    match account {
        Some(acc) => Ok(u256_to_hex(acc.balance)),
        None => Ok("0x0".to_string()),
    }
}

/// Set account balance
pub unsafe fn set_balance_impl(
    instance: &mut RevmInstance,
    address: *const c_char,
    balance: *const c_char,
) -> Result<()> {
    let addr = hex_to_address(&c_str_to_string(address)?)?;
    let balance_val = hex_to_u256(&c_str_to_string(balance)?)?;
    
    // Access the database through the journal
    let db = instance.evm.ctx().journal().db();
    db.insert_account_info(addr, revm::state::AccountInfo {
        balance: balance_val,
        nonce: 0,
        code_hash: revm::primitives::KECCAK_EMPTY,
        code: Some(revm::bytecode::Bytecode::default()),
    });
    
    Ok(())
}

/// Get storage value
pub unsafe fn get_storage_impl(
    instance: &mut RevmInstance,
    address: *const c_char,
    slot: *const c_char,
) -> Result<String> {
    let addr = hex_to_address(&c_str_to_string(address)?)?;
    let slot_u256 = hex_to_u256(&c_str_to_string(slot)?)?;
    
    let value = instance.evm.ctx().journal().db().storage(addr, slot_u256)?;
    Ok(u256_to_hex(value))
}

/// Set storage value
pub unsafe fn set_storage_impl(
    instance: &mut RevmInstance,
    address: *const c_char,
    slot: *const c_char,
    value: *const c_char,
) -> Result<()> {
    let addr = hex_to_address(&c_str_to_string(address)?)?;
    let slot_u256 = hex_to_u256(&c_str_to_string(slot)?)?;
    let value_u256 = hex_to_u256(&c_str_to_string(value)?)?;
    
    let db = instance.evm.ctx().journal().db();
    db.insert_account_storage(addr, slot_u256, value_u256)?;
    
    Ok(())
}

/// Set account nonce
pub unsafe fn set_nonce_impl(
    instance: &mut RevmInstance,
    address: *const c_char,
    nonce: u64,
) -> Result<()> {
    let addr = hex_to_address(&c_str_to_string(address)?)?;
    
    // Get existing account info or create new one
    let db = instance.evm.ctx().journal().db();
    let existing_account = db.basic(addr)?;
    
    let account_info = match existing_account {
        Some(mut acc) => {
            acc.nonce = nonce;
            acc
        }
        None => revm::state::AccountInfo {
            balance: U256::ZERO,
            nonce,
            code_hash: revm::primitives::KECCAK_EMPTY,
            code: Some(revm::bytecode::Bytecode::default()),
        },
    };
    
    db.insert_account_info(addr, account_info);
    Ok(())
}

/// Get account nonce
pub unsafe fn get_nonce_impl(
    instance: &mut RevmInstance,
    address: *const c_char,
) -> Result<u64> {
    let addr = hex_to_address(&c_str_to_string(address)?)?;
    let account = instance.evm.ctx().journal().db().basic(addr)?;
    
    match account {
        Some(acc) => Ok(acc.nonce),
        None => Ok(0),
    }
}

/// Transfer ETH between accounts
pub unsafe fn transfer_impl(
    instance: &mut RevmInstance,
    from: *const c_char,
    to: *const c_char,
    value: *const c_char,
    gas_limit: u64,
) -> Result<ExecutionResultFFI> {
    let from_addr = hex_to_address(&c_str_to_string(from)?)?;
    let to_addr = hex_to_address(&c_str_to_string(to)?)?;
    let value_u256 = hex_to_u256(&c_str_to_string(value)?)?;

    // Get the chain ID from the context
    let chain_id = instance.evm.ctx.cfg.chain_id;
    
    // Get the current nonce for the caller
    let current_nonce = {
        let account = instance.evm.ctx().journal().db().basic(from_addr)?;
        match account {
            Some(acc) => acc.nonce,
            None => 0,
        }
    };

    instance.evm.ctx().modify_tx(|tx| {
        tx.caller = from_addr;
        tx.kind = TxKind::Call(to_addr);
        tx.value = value_u256;
        tx.data = Bytes::new();
        tx.gas_limit = gas_limit;
        tx.gas_price = 1_000_000_000u128; // 1 gwei
        tx.nonce = current_nonce;
        tx.chain_id = Some(chain_id);
    });

    let result = instance.evm.replay_commit()?;
    Ok(convert_execution_result(result))
}

/// Call a contract
pub unsafe fn call_contract_impl(
    instance: &mut RevmInstance,
    from: *const c_char,
    to: *const c_char,
    data: *const u8,
    data_len: c_uint,
    value: *const c_char,
    gas_limit: u64,
) -> Result<ExecutionResultFFI> {
    let from_addr = hex_to_address(&c_str_to_string(from)?)?;
    let to_addr = hex_to_address(&c_str_to_string(to)?)?;
    
    let value_u256 = if value.is_null() {
        U256::ZERO
    } else {
        hex_to_u256(&c_str_to_string(value)?)?
    };
    
    let call_data = if data.is_null() || data_len == 0 {
        Bytes::new()
    } else {
        let slice = slice::from_raw_parts(data, data_len as usize);
        Bytes::copy_from_slice(slice)
    };

    // Get the chain ID from the context
    let chain_id = instance.evm.ctx.cfg.chain_id;
    
    // Get the current nonce for the caller
    let current_nonce = {
        let account = instance.evm.ctx().journal().db().basic(from_addr)?;
        match account {
            Some(acc) => acc.nonce,
            None => 0,
        }
    };

    instance.evm.ctx().modify_tx(|tx| {
        tx.caller = from_addr;
        tx.kind = TxKind::Call(to_addr);
        tx.value = value_u256;
        tx.data = call_data;
        tx.gas_limit = gas_limit;
        tx.gas_price = 1_000_000_000u128; // 1 gwei
        tx.nonce = current_nonce;
        tx.chain_id = Some(chain_id);
    });

    let result = instance.evm.replay_commit()?;
    Ok(convert_execution_result(result))
}

/// Call a contract (view call - doesn't commit state)
pub unsafe fn view_call_contract_impl(
    instance: &mut RevmInstance,
    from: *const c_char,
    to: *const c_char,
    data: *const u8,
    data_len: c_uint,
    gas_limit: u64,
) -> Result<ExecutionResultFFI> {
    let from_addr = hex_to_address(&c_str_to_string(from)?)?;
    let to_addr = hex_to_address(&c_str_to_string(to)?)?;
    
    let call_data = if data.is_null() || data_len == 0 {
        Bytes::new()
    } else {
        let slice = slice::from_raw_parts(data, data_len as usize);
        Bytes::copy_from_slice(slice)
    };

    // Get the chain ID from the context
    let chain_id = instance.evm.ctx.cfg.chain_id;
    
    // Get the current nonce for the caller
    let current_nonce = {
        let account = instance.evm.ctx().journal().db().basic(from_addr)?;
        match account {
            Some(acc) => acc.nonce,
            None => 0,
        }
    };

    instance.evm.ctx().modify_tx(|tx| {
        tx.caller = from_addr;
        tx.kind = TxKind::Call(to_addr);
        tx.value = U256::ZERO; // View calls don't transfer value
        tx.data = call_data;
        tx.gas_limit = gas_limit;
        tx.gas_price = 1_000_000_000u128; // 1 gwei
        tx.nonce = current_nonce;
        tx.chain_id = Some(chain_id);
    });

    // Use replay() instead of replay_commit() for view calls
    let result = instance.evm.replay()?;
    Ok(convert_execution_result(result.result))
} 