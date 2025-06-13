use revm::{
    primitives::{address, Address, Bytes, TxKind, U256, hex},
    Context, MainBuilder, MainContext, ExecuteEvm,
    context::{TxEnv, ContextTr},
    context_interface::JournalTr,
    handler::{MainnetEvm, MainnetContext},
    database_interface::EmptyDB,
    bytecode::Bytecode,
};
use std::fs;
use std::time::{Instant, Duration};

// Function selectors
const MINT_SELECTOR: [u8; 4] = [0x40, 0xc1, 0x0f, 0x19]; // mint(address,uint256)
const BALANCE_OF_SELECTOR: [u8; 4] = [0x70, 0xa0, 0x82, 0x31]; // balanceOf(address)
const BATCH_TRANSFER_SELECTOR: [u8; 4] = [0x1b, 0xc9, 0x2c, 0xf4]; // batchTransferSequential(address,uint256,uint256)

// Addresses
const ALICE: Address = address!("1000000000000000000000000000000000000001");
const BIGA_CONTRACT: Address = address!("2000000000000000000000000000000000000001");

type MyEvm = MainnetEvm<MainnetContext<EmptyDB>>;

fn main() {
    println!("🚀 Pure REVM Benchmark - BIGA Token Batch Transfers");
    
    // Load BIGA contract bytecode
    let biga_bytecode = load_bytecode("../bytecode/BIGA.bin");
    
    // Initialize EVM
    let mut evm = Context::mainnet().build_mainnet();
    let mut alice_nonce = 0u64;
    
    // Deploy BIGA contract
    println!("📦 Deploying BIGA contract...");
    deploy_contract(&mut evm, BIGA_CONTRACT, &biga_bytecode, &mut alice_nonce);
    
    // Mint tokens to Alice
    println!("💰 Minting tokens to Alice...");
    let total_tokens = U256::from(10000000) * U256::from(1000000000000000000u64); // 10M tokens (enough for 50k transfers)
    mint_tokens(&mut evm, ALICE, total_tokens, &mut alice_nonce);
    
    // Verify Alice's balance
    let alice_balance = get_token_balance(&mut evm, ALICE, &mut alice_nonce);
    println!("✅ Alice's balance: {} tokens", alice_balance / U256::from(1000000000000000000u64));
    
    // Perform batch transfers
    println!("🔄 Performing batch transfers...");
    let num_transfers = 50000u64;
    let duration = perform_batch_transfers(&mut evm, num_transfers, &mut alice_nonce);
    
    // Calculate performance metrics
    let transfers_per_second = num_transfers as f64 / duration.as_secs_f64();
    
    println!("⚡ Benchmark Results:");
    println!("   Transfers: {}", num_transfers);
    println!("   Duration: {:.2}ms", duration.as_millis());
    println!("   Transfers/sec: {:.2}", transfers_per_second);
    
    // Verify some recipient balances
    println!("🔍 Verifying transfers...");
    let start_recipient = U256::from_str_radix("3000000000000000000000000000000000000001", 16).unwrap();
    for i in 0..3 {
        let recipient = Address::from_slice(&(start_recipient + U256::from(i)).to_be_bytes::<32>()[12..]);
        let balance = get_token_balance(&mut evm, recipient, &mut alice_nonce);
        println!("   Recipient {}: {} tokens", i + 1, balance / U256::from(1000000000000000000u64));
    }
    
    // Verify Alice's final balance
    let alice_final_balance = get_token_balance(&mut evm, ALICE, &mut alice_nonce);
    println!("   Alice final balance: {} tokens", alice_final_balance / U256::from(1000000000000000000u64));
    
    println!("✨ Benchmark completed successfully!");
}

fn load_bytecode(path: &str) -> Bytes {
    let bytecode_str = fs::read_to_string(path)
        .expect("Failed to read bytecode file");
    let bytecode_str = bytecode_str.trim();
    
    if bytecode_str.starts_with("0x") {
        hex::decode(&bytecode_str[2..]).expect("Invalid hex in bytecode").into()
    } else {
        hex::decode(bytecode_str).expect("Invalid hex in bytecode").into()
    }
}

fn deploy_contract(evm: &mut MyEvm, contract_address: Address, bytecode: &Bytes, alice_nonce: &mut u64) {
    let tx = TxEnv {
        caller: ALICE,
        kind: TxKind::Create,
        data: bytecode.clone(),
        value: U256::ZERO,
        gas_limit: 10_000_000,
        nonce: *alice_nonce,
        ..Default::default()
    };

    let result = evm.transact(tx).unwrap();
    if !result.is_success() {
        panic!("Contract deployment failed: {:?}", result);
    }

    *alice_nonce += 1;

    // Set the contract address in the database
    let account = evm.journal().load_account(contract_address).unwrap();
    account.data.info.code = Some(Bytecode::new_legacy(result.output().unwrap().clone()));
}

fn mint_tokens(evm: &mut MyEvm, to: Address, amount: U256, alice_nonce: &mut u64) {
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&MINT_SELECTOR);
    calldata.extend_from_slice(&[0u8; 12]); // padding for address
    calldata.extend_from_slice(to.as_slice());
    calldata.extend_from_slice(&amount.to_be_bytes::<32>());

    let tx = TxEnv {
        caller: ALICE,
        kind: TxKind::Call(BIGA_CONTRACT),
        data: Bytes::from(calldata),
        value: U256::ZERO,
        gas_limit: 1_000_000,
        nonce: *alice_nonce,
        ..Default::default()
    };

    let result = evm.transact(tx).unwrap();
    if !result.is_success() {
        panic!("Mint failed: {:?}", result);
    }

    *alice_nonce += 1;
}

fn get_token_balance(evm: &mut MyEvm, account: Address, alice_nonce: &mut u64) -> U256 {
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&BALANCE_OF_SELECTOR);
    calldata.extend_from_slice(&[0u8; 12]); // padding for address
    calldata.extend_from_slice(account.as_slice());

    let tx = TxEnv {
        caller: ALICE,
        kind: TxKind::Call(BIGA_CONTRACT),
        data: Bytes::from(calldata),
        value: U256::ZERO,
        gas_limit: 1_000_000,
        nonce: *alice_nonce,
        ..Default::default()
    };

    let result = evm.transact(tx).unwrap();
    if !result.is_success() {
        panic!("Balance query failed: {:?}", result);
    }

    *alice_nonce += 1;

    let output = result.output().unwrap();
    if output.len() >= 32 {
        U256::from_be_slice(&output[..32])
    } else {
        U256::ZERO
    }
}

fn perform_batch_transfers(evm: &mut MyEvm, num_transfers: u64, alice_nonce: &mut u64) -> Duration {
    let start_recipient = U256::from_str_radix("3000000000000000000000000000000000000001", 16).unwrap();
    let amount_per_transfer = U256::from(1000000000000000000u64); // 1 token

    let mut calldata = Vec::new();
    calldata.extend_from_slice(&BATCH_TRANSFER_SELECTOR);
    calldata.extend_from_slice(&start_recipient.to_be_bytes::<32>());
    calldata.extend_from_slice(&U256::from(num_transfers).to_be_bytes::<32>());
    calldata.extend_from_slice(&amount_per_transfer.to_be_bytes::<32>());

    let tx = TxEnv {
        caller: ALICE,
        kind: TxKind::Call(BIGA_CONTRACT),
        data: Bytes::from(calldata),
        value: U256::ZERO,
        gas_limit: 2_000_000_000, // Higher gas limit for 50k transfers
        nonce: *alice_nonce,
        ..Default::default()
    };

    let start_time = Instant::now();
    let result = evm.transact(tx).unwrap();
    let duration = start_time.elapsed();

    if !result.is_success() {
        panic!("Batch transfer failed: {:?}", result);
    }

    *alice_nonce += 1;

    duration
} 