use revm_ffi::{
    revm_set_tx, revm_execute, revm_free_execution_result,
    revm_set_balance, revm_set_code, RevmConfigFFI, revm_new_with_config,
};
use std::ffi::CString;

#[test]
fn refund_is_non_zero_pre_london() {
    unsafe {
        // Build custom config targeting Berlin (spec_id 11) to ensure refunds are enabled.
        let config = revm_ffi::RevmConfigFFI {
            chain_id: 1,
            spec_id: 11, // Berlin hard fork
            disable_nonce_check: false,
            disable_balance_check: false,
            disable_block_gas_limit: false,
            disable_base_fee: false,
            disable_eip3607: true,
            max_code_size: 0,
        };

        let inst = revm_new_with_config(&config);
        assert!(!inst.is_null());

        // Prepare runtime code (CALLER SELFDESTRUCT)
        let bytecode = vec![0x33u8, 0xFFu8];
        let from = CString::new("0x71562b71999873DB5b286dF957af199Ec94617F7").unwrap();
        let to = CString::new("0x000000000000000000000000000000000000aaaa").unwrap();

        // Fund the sender
        let _ = revm_set_balance(inst, from.as_ptr(), CString::new("0x1000000000000000").unwrap().as_ptr());

        // Inject code into 0xAAAA
        revm_set_code(inst, to.as_ptr(), bytecode.as_ptr(), bytecode.len() as u32);

        // Now call SELFDESTRUCT contract (address 0xAAAA)
        let output: [u8; 0] = [];
        let ok = revm_set_tx(
            inst,
            from.as_ptr(),
            to.as_ptr(),
            CString::new("0x1").unwrap().as_ptr(),
            output.as_ptr(),
            0, // data_len
            60000,
            CString::new("0x1").unwrap().as_ptr(),
            0,
        );
        assert_eq!(ok, 0);
        let exec_res = revm_execute(inst);
        assert!(!exec_res.is_null());
        let gas_refunded = (*exec_res).gas_refunded;
        let gas_used = (*exec_res).gas_used;
        println!("gas_used {}, gas_refunded {}", gas_used, gas_refunded);
        assert!(gas_refunded > 0, "refund should be >0, got {}", gas_refunded);
        assert_eq!(gas_used, gas_refunded, "pre-London cap should refund all but intrinsic execution gas");
        revm_free_execution_result(exec_res);
    }
} 