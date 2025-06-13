package main

import (
	"encoding/hex"
	"fmt"
	"io/ioutil"
	"math/big"
	"strings"
	"time"

	"github.com/ethereum/go-ethereum/common"
	"github.com/ethereum/go-ethereum/core"
	"github.com/ethereum/go-ethereum/core/rawdb"
	"github.com/ethereum/go-ethereum/core/state"
	"github.com/ethereum/go-ethereum/core/tracing"
	"github.com/ethereum/go-ethereum/core/vm"
	"github.com/ethereum/go-ethereum/params"
	"github.com/ethereum/go-ethereum/triedb"
	"github.com/holiman/uint256"
)

// Function selectors (same as REVM benchmark)
var (
	mintSelector         = []byte{0x40, 0xc1, 0x0f, 0x19} // mint(address,uint256)
	balanceOfSelector    = []byte{0x70, 0xa0, 0x82, 0x31} // balanceOf(address)
	batchTransferSelector = []byte{0x1b, 0xc9, 0x2c, 0xf4} // batchTransferSequential(address,uint256,uint256)
)

// Addresses (same as REVM benchmark)
var (
	aliceAddr     = common.HexToAddress("0x1000000000000000000000000000000000000001")
	bigaContract  = common.HexToAddress("0x2000000000000000000000000000000000000001")
)

func main() {
	fmt.Println("🚀 Pure BSC-EVM Benchmark - BIGA Token Batch Transfers")

	// Load BIGA contract bytecode
	bigaBytecode := loadBytecode("../bytecode/BIGA.bin")

	// Initialize EVM with BSC configuration
	db := rawdb.NewMemoryDatabase()
	trieDB := triedb.NewDatabase(db, nil)
	statedb, _ := state.New(common.Hash{}, state.NewDatabase(trieDB, nil))
	
	// Create Alice account with some BNB for gas
	statedb.CreateAccount(aliceAddr)
	aliceBalance := uint256.NewInt(1000000000000000000) // 1 BNB
	statedb.SetBalance(aliceAddr, aliceBalance, tracing.BalanceChangeUnspecified)
	
	// Create EVM context with BSC parameters
	chainConfig := &params.ChainConfig{
		ChainID:                       big.NewInt(97), // BSC Testnet
		HomesteadBlock:                big.NewInt(0),
		DAOForkBlock:                  nil,
		DAOForkSupport:                false,
		EIP150Block:                   big.NewInt(0),
		EIP155Block:                   big.NewInt(0),
		EIP158Block:                   big.NewInt(0),
		ByzantiumBlock:                big.NewInt(0),
		ConstantinopleBlock:           big.NewInt(0),
		PetersburgBlock:               big.NewInt(0),
		IstanbulBlock:                 big.NewInt(0),
		MuirGlacierBlock:              big.NewInt(0),
		BerlinBlock:                   big.NewInt(0),
		LondonBlock:                   big.NewInt(0),
		ArrowGlacierBlock:             big.NewInt(0),
		GrayGlacierBlock:              big.NewInt(0),
		MergeNetsplitBlock:            big.NewInt(0),
		ShanghaiTime:                  new(uint64), // Enable Shanghai for PUSH0
		CancunTime:                    new(uint64), // Enable Cancun for PUSH0
	}
	vmConfig := vm.Config{}
	
	blockContext := vm.BlockContext{
		CanTransfer: core.CanTransfer,
		Transfer:    core.Transfer,
		GetHash:     func(uint64) common.Hash { return common.Hash{} },
		Coinbase:    common.Address{},
		BlockNumber: big.NewInt(1),
		Time:        uint64(1681338455), // Set to a time after Shanghai activation
		Difficulty:  big.NewInt(1),
		GasLimit:    10000000000, // 10B gas limit
		BaseFee:     big.NewInt(0), // BSC has 0 base fee
	}

	// Create EVM
	evm := vm.NewEVM(blockContext, statedb, chainConfig, vmConfig)

	// Deploy BIGA contract
	fmt.Println("📦 Deploying BIGA contract...")
	deployContract(evm, bigaBytecode)

	// Mint tokens to Alice
	fmt.Println("💰 Minting tokens to Alice...")
	totalTokens := new(big.Int).Mul(big.NewInt(10000000), big.NewInt(1000000000000000000)) // 10M tokens
	mintTokens(evm, aliceAddr, totalTokens)

	// Verify Alice's balance
	aliceTokenBalance := getTokenBalance(evm, aliceAddr)
	fmt.Printf("✅ Alice's balance: %s tokens\n", new(big.Int).Div(aliceTokenBalance, big.NewInt(1000000000000000000)).String())

	// Perform batch transfers
	fmt.Println("🔄 Performing batch transfers...")
	numTransfers := int64(50000)
	duration := performBatchTransfers(evm, numTransfers)

	// Calculate performance metrics
	transfersPerSecond := float64(numTransfers) / duration.Seconds()

	fmt.Println("⚡ BSC-EVM Benchmark Results:")
	fmt.Printf("   Transfers: %d\n", numTransfers)
	fmt.Printf("   Duration: %.2fms\n", float64(duration.Nanoseconds())/1000000)
	fmt.Printf("   Transfers/sec: %.2f\n", transfersPerSecond)

	// Verify some recipient balances
	fmt.Println("🔍 Verifying transfers...")
	startRecipient := common.HexToAddress("0x3000000000000000000000000000000000000001")
	for i := 0; i < 3; i++ {
		recipient := common.BigToAddress(new(big.Int).Add(startRecipient.Big(), big.NewInt(int64(i))))
		balance := getTokenBalance(evm, recipient)
		fmt.Printf("   Recipient %d: %s tokens\n", i+1, new(big.Int).Div(balance, big.NewInt(1000000000000000000)).String())
	}

	// Verify Alice's final balance
	aliceFinalBalance := getTokenBalance(evm, aliceAddr)
	fmt.Printf("   Alice final balance: %s tokens\n", new(big.Int).Div(aliceFinalBalance, big.NewInt(1000000000000000000)).String())

	fmt.Println("✨ BSC-EVM Benchmark completed successfully!")
}

func loadBytecode(path string) []byte {
	data, err := ioutil.ReadFile(path)
	if err != nil {
		panic(fmt.Sprintf("Failed to read bytecode file: %v", err))
	}

	bytecodeStr := strings.TrimSpace(string(data))
	if strings.HasPrefix(bytecodeStr, "0x") {
		bytecodeStr = bytecodeStr[2:]
	}

	bytecode, err := hex.DecodeString(bytecodeStr)
	if err != nil {
		panic(fmt.Sprintf("Invalid hex in bytecode: %v", err))
	}

	return bytecode
}

func deployContract(evm *vm.EVM, bytecode []byte) {
	// Create a contract reference for Alice
	caller := vm.AccountRef(aliceAddr)

	// Deploy contract
	value := uint256.NewInt(0)
	ret, contractAddr, leftOverGas, err := evm.Create(caller, bytecode, 10000000, value)
	if err != nil {
		panic(fmt.Sprintf("Contract deployment failed: %v", err))
	}

	fmt.Printf("Contract deployed at: %s, gas used: %d\n", contractAddr.Hex(), 10000000-leftOverGas)
	
	// Set the contract code at our expected address
	evm.StateDB.SetCode(bigaContract, ret)
}

func mintTokens(evm *vm.EVM, to common.Address, amount *big.Int) {
	// Prepare calldata
	calldata := make([]byte, 0, 68)
	calldata = append(calldata, mintSelector...)
	calldata = append(calldata, make([]byte, 12)...) // padding for address
	calldata = append(calldata, to.Bytes()...)
	calldata = append(calldata, common.LeftPadBytes(amount.Bytes(), 32)...)

	// Execute transaction
	executeTransaction(evm, bigaContract, calldata, 1000000)
}

func getTokenBalance(evm *vm.EVM, account common.Address) *big.Int {
	// Prepare calldata
	calldata := make([]byte, 0, 36)
	calldata = append(calldata, balanceOfSelector...)
	calldata = append(calldata, make([]byte, 12)...) // padding for address
	calldata = append(calldata, account.Bytes()...)

	// Execute transaction
	ret := executeTransaction(evm, bigaContract, calldata, 1000000)

	if len(ret) >= 32 {
		return new(big.Int).SetBytes(ret[:32])
	}
	return big.NewInt(0)
}

func performBatchTransfers(evm *vm.EVM, numTransfers int64) time.Duration {
	startRecipient := common.HexToAddress("0x3000000000000000000000000000000000000001")
	amountPerTransfer := big.NewInt(1000000000000000000) // 1 token

	// Prepare calldata
	calldata := make([]byte, 0, 100)
	calldata = append(calldata, batchTransferSelector...)
	calldata = append(calldata, common.LeftPadBytes(startRecipient.Bytes(), 32)...)
	calldata = append(calldata, common.LeftPadBytes(big.NewInt(numTransfers).Bytes(), 32)...)
	calldata = append(calldata, common.LeftPadBytes(amountPerTransfer.Bytes(), 32)...)

	// Measure execution time
	startTime := time.Now()
	executeTransaction(evm, bigaContract, calldata, 2000000000)
	duration := time.Since(startTime)

	return duration
}

func executeTransaction(evm *vm.EVM, to common.Address, data []byte, gasLimit uint64) []byte {
	// Create a contract reference for Alice
	caller := vm.AccountRef(aliceAddr)

	// Execute call
	value := uint256.NewInt(0)
	ret, leftOverGas, err := evm.Call(caller, to, data, gasLimit, value)
	if err != nil {
		panic(fmt.Sprintf("Transaction failed: %v", err))
	}

	fmt.Printf("Transaction executed, gas used: %d\n", gasLimit-leftOverGas)
	return ret
} 