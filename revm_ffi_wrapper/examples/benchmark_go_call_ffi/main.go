package main

/*
#cgo LDFLAGS: -L../../target/release -lrevm_ffi
#include "../../revm_ffi.h"
#include <stdlib.h>
*/
import "C"
import (
	"encoding/hex"
	"fmt"
	"io/ioutil"
	"math/big"
	"strings"
	"time"
	"unsafe"

	"github.com/ethereum/go-ethereum/common"
)

// Function selectors (same as other benchmarks)
var (
	mintSelector          = []byte{0x40, 0xc1, 0x0f, 0x19} // mint(address,uint256)
	balanceOfSelector     = []byte{0x70, 0xa0, 0x82, 0x31} // balanceOf(address)
	transferSelector      = []byte{0xa9, 0x05, 0x9c, 0xbb} // transfer(address,uint256)
	batchTransferSelector = []byte{0x1b, 0xc9, 0x2c, 0xf4} // batchTransferSequential(address,uint256,uint256)
)

// Addresses (same as other benchmarks)
var (
	aliceAddr    = "0x1000000000000000000000000000000000000001"
	bobAddr      = "0x2000000000000000000000000000000000000001"
	charlieAddr  = "0x3000000000000000000000000000000000000001"
	bigaContract string // Will be set after deployment
)

func main() {
	fmt.Println("🚀 Pure REVM FFI Benchmark - Batch Token Transfers (BIGA)")

	// Load BIGA contract bytecode
	bigaBytecode := loadBytecode("../bytecode/BIGA.bin")

	// Create REVM instance
	fmt.Print("🔧 Creating REVM instance... ")
	instance := C.revm_new()
	if instance == nil {
		fmt.Println("❌ Failed to create REVM instance")
		return
	}
	defer C.revm_free(instance)
	fmt.Println("✅")

	// Setup Alice account with some ETH for gas
	fmt.Print("💰 Setting up Alice account... ")
	if !setBalance(instance, aliceAddr, "0x8ac7230489e80000") { // 10 ETH
		fmt.Println("❌ Failed to set Alice balance")
		return
	}
	fmt.Println("✅")

	// Deploy BIGA contract
	fmt.Print("📦 Deploying BIGA contract... ")
	deployTime := time.Now()
	contractAddr := deployContract(instance, aliceAddr, bigaBytecode)
	if contractAddr == "" {
		fmt.Println("❌ Failed to deploy BIGA contract")
		return
	}
	bigaContract = contractAddr
	fmt.Printf("✅ %s (%v)\n", contractAddr, time.Since(deployTime))

	// Mint tokens to Alice
	fmt.Print("💰 Minting tokens to Alice... ")
	totalSupply := new(big.Int).Mul(big.NewInt(10000000), big.NewInt(1000000000000000000)) // 10M tokens with 18 decimals
	if !mintTokens(instance, aliceAddr, totalSupply) {
		fmt.Println("❌ Failed to mint tokens")
		return
	}
	
	aliceBalance := getTokenBalance(instance, aliceAddr)
	fmt.Printf("✅ Alice's balance: %s tokens\n", formatTokenAmount(aliceBalance))

	// Perform batch transfers
	fmt.Println("🔄 Performing batch transfers...")
	transferCount := 50000 // Full benchmark with 50k transfers
	amountPerTransfer := big.NewInt(1000000000000000000) // 1 token with 18 decimals
	startRecipient := common.HexToAddress("0x2000000000000000000000000000000000000001")
	
	fmt.Printf("   Alice balance before: %s tokens\n", formatTokenAmount(getTokenBalance(instance, aliceAddr)))
	
	benchmarkTime := time.Now()
	success := performBatchTransfers(instance, startRecipient, transferCount, amountPerTransfer)
	duration := time.Since(benchmarkTime)
	
	if !success {
		fmt.Println("❌ Batch transfers failed")
		return
	}
	
	fmt.Printf("✅ Batch transfer transaction succeeded\n")

	transfersPerSec := float64(transferCount) / duration.Seconds()
	fmt.Printf("⚡ REVM FFI Benchmark Results:\n")
	fmt.Printf("   Transfers: %d\n", transferCount)
	fmt.Printf("   Duration: %v\n", duration)
	fmt.Printf("   Transfers/sec: %.2f\n", transfersPerSec)

	// Verify transfers
	fmt.Println("🔍 Verifying transfers...")
	aliceFinalBalance := getTokenBalance(instance, aliceAddr)
	fmt.Printf("   Alice final balance: %s tokens\n", formatTokenAmount(aliceFinalBalance))
	
	// Check a few recipient balances
	for i := 0; i < 3; i++ {
		recipient := common.BigToAddress(new(big.Int).Add(startRecipient.Big(), big.NewInt(int64(i))))
		balance := getTokenBalance(instance, recipient.Hex())
		fmt.Printf("   Recipient %d (%s): %s tokens\n", i+1, recipient.Hex(), formatTokenAmount(balance))
	}

	fmt.Println("✨ REVM FFI Benchmark completed successfully!")
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

func setBalance(instance *C.RevmInstance, address, balance string) bool {
	addressCStr := C.CString(address)
	balanceCStr := C.CString(balance)
	defer C.free(unsafe.Pointer(addressCStr))
	defer C.free(unsafe.Pointer(balanceCStr))

	result := C.revm_set_balance(instance, addressCStr, balanceCStr)
	return result == 0
}

func deployContract(instance *C.RevmInstance, deployer string, bytecode []byte) string {
	deployerCStr := C.CString(deployer)
	defer C.free(unsafe.Pointer(deployerCStr))

	// Deploy the contract using the FFI function
	result := C.revm_deploy_contract(
		instance,
		deployerCStr,
		(*C.uchar)(unsafe.Pointer(&bytecode[0])),
		C.uint(len(bytecode)),
		C.uint64_t(10000000), // 10M gas limit
	)

	if result == nil {
		// Get the error message
		errorPtr := C.revm_get_last_error(instance)
		if errorPtr != nil {
			errorMsg := C.GoString(errorPtr)
			fmt.Printf("Failed to deploy contract: %s\n", errorMsg)
		} else {
			fmt.Printf("Failed to deploy contract: unknown error\n")
		}
		return ""
	}
	defer C.revm_free_deployment_result(result)

	if result.success != 1 {
		fmt.Printf("Contract deployment failed (success=%d, gas_used=%d)\n", result.success, result.gas_used)
		return ""
	}

	if result.contract_address == nil {
		fmt.Printf("Contract deployment succeeded but no address returned\n")
		return ""
	}

	address := C.GoString(result.contract_address)
	return address
}

func mintTokens(instance *C.RevmInstance, to string, amount *big.Int) bool {
	// Prepare calldata for mint(address,uint256)
	calldata := make([]byte, 0, 68)
	calldata = append(calldata, mintSelector...)
	calldata = append(calldata, make([]byte, 12)...) // padding for address
	calldata = append(calldata, common.HexToAddress(to).Bytes()...)
	calldata = append(calldata, common.LeftPadBytes(amount.Bytes(), 32)...)

	// Execute the mint transaction - Alice should be the caller
	return executeTransaction(instance, aliceAddr, bigaContract, calldata, "0x0", 1000000)
}

func getTokenBalance(instance *C.RevmInstance, account string) string {
	// Prepare calldata for balanceOf(address)
	calldata := make([]byte, 0, 36)
	calldata = append(calldata, balanceOfSelector...)
	calldata = append(calldata, make([]byte, 12)...) // padding for address
	calldata = append(calldata, common.HexToAddress(account).Bytes()...)

	// Execute the balanceOf call - use Alice as caller (like BSC FFI benchmark uses DEPLOYER_ADDRESS)
	fromCStr := C.CString(aliceAddr)
	toCStr := C.CString(bigaContract)
	valueCStr := C.CString("0x0")

	defer C.free(unsafe.Pointer(fromCStr))
	defer C.free(unsafe.Pointer(toCStr))
	defer C.free(unsafe.Pointer(valueCStr))

	result := C.revm_call_contract(
		instance,
		fromCStr,
		toCStr,
		(*C.uchar)(unsafe.Pointer(&calldata[0])),
		C.uint(len(calldata)),
		valueCStr,
		C.uint64_t(1000000),
	)

	if result == nil || result.success != 1 {
		if result != nil {
			C.revm_free_execution_result(result)
		}
		return "0x0"
	}
	defer C.revm_free_execution_result(result)

	if result.output_len == 0 {
		return "0x0"
	}

	// Convert output to hex string - use the exact same pattern as BSC FFI benchmark
	outputSlice := (*[32]byte)(unsafe.Pointer(result.output_data))[:result.output_len:result.output_len]
	return "0x" + hex.EncodeToString(outputSlice)
}

func performBatchTransfers(instance *C.RevmInstance, startRecipient common.Address, transferCount int, amountPerTransfer *big.Int) bool {
	// Prepare calldata for batchTransferSequential(address,uint256,uint256)
	calldata := make([]byte, 0, 100)
	calldata = append(calldata, batchTransferSelector...)
	
	// Add startRecipient address (32 bytes)
	calldata = append(calldata, make([]byte, 12)...) // padding for address
	calldata = append(calldata, startRecipient.Bytes()...)
	
	// Add transferCount (32 bytes)
	transferCountBytes := make([]byte, 32)
	big.NewInt(int64(transferCount)).FillBytes(transferCountBytes)
	calldata = append(calldata, transferCountBytes...)
	
	// Add amountPerTransfer (32 bytes)
	amountBytes := make([]byte, 32)
	amountPerTransfer.FillBytes(amountBytes)
	calldata = append(calldata, amountBytes...)

	// Execute the batch transfer transaction
	return executeTransaction(instance, aliceAddr, bigaContract, calldata, "0x0", 2000000000) // 2B gas limit for 50k transfers
}

func transferTokens(instance *C.RevmInstance, from, to string, amount *big.Int) bool {
	// Prepare calldata for transfer(address,uint256)
	calldata := make([]byte, 0, 68)
	calldata = append(calldata, transferSelector...)
	calldata = append(calldata, make([]byte, 12)...) // padding for address
	calldata = append(calldata, common.HexToAddress(to).Bytes()...)
	calldata = append(calldata, common.LeftPadBytes(amount.Bytes(), 32)...)

	// Execute the transfer transaction
	return executeTransaction(instance, from, bigaContract, calldata, "0x0", 100000)
}

func executeTransaction(instance *C.RevmInstance, from, to string, calldata []byte, value string, gasLimit uint64) bool {
	fromCStr := C.CString(from)
	toCStr := C.CString(to)
	valueCStr := C.CString(value)

	defer C.free(unsafe.Pointer(fromCStr))
	defer C.free(unsafe.Pointer(toCStr))
	defer C.free(unsafe.Pointer(valueCStr))

	result := C.revm_call_contract(
		instance,
		fromCStr,
		toCStr,
		(*C.uchar)(unsafe.Pointer(&calldata[0])),
		C.uint(len(calldata)),
		valueCStr,
		C.uint64_t(gasLimit),
	)

	if result == nil {
		fmt.Printf("❌ Transaction failed: result is nil\n")
		return false
	}
	defer C.revm_free_execution_result(result)

	success := result.success == 1
	fmt.Printf("🔍 Transaction result: success=%d, gas_used=%d\n", result.success, result.gas_used)
	
	if !success {
		// Try to get error message
		errorPtr := C.revm_get_last_error(instance)
		if errorPtr != nil {
			errorMsg := C.GoString(errorPtr)
			fmt.Printf("❌ Transaction error: %s\n", errorMsg)
		}
	}

	return success
}

func formatTokenAmount(hexAmount string) string {
	if hexAmount == "" || hexAmount == "0x" || hexAmount == "0x0" {
		return "0"
	}

	// Remove 0x prefix if present
	if strings.HasPrefix(hexAmount, "0x") {
		hexAmount = hexAmount[2:]
	}

	// Parse as big integer
	amount := new(big.Int)
	amount.SetString(hexAmount, 16)

	// Convert from wei to tokens (divide by 10^18)
	divisor := new(big.Int).Exp(big.NewInt(10), big.NewInt(18), nil)
	tokens := new(big.Int).Div(amount, divisor)

	return tokens.String()
}

func generateRecipientAddress(startRecipient string, offset int) string {
	startAddr := common.HexToAddress(startRecipient)
	recipientBig := new(big.Int).SetBytes(startAddr.Bytes())
	recipientBig.Add(recipientBig, big.NewInt(int64(offset)))
	
	// Convert back to address
	recipientBytes := recipientBig.Bytes()
	if len(recipientBytes) > 20 {
		recipientBytes = recipientBytes[len(recipientBytes)-20:]
	}
	
	var addr common.Address
	copy(addr[20-len(recipientBytes):], recipientBytes)
	
	return addr.Hex()
} 