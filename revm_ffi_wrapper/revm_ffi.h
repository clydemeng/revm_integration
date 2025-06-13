#ifndef REVM_FFI_H
#define REVM_FFI_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// Forward declarations
typedef struct RevmInstance RevmInstance;

// Configuration for REVM instance creation
typedef struct {
    uint64_t chain_id;                  // Chain ID (1 for Ethereum mainnet, 56 for BSC mainnet, 97 for BSC testnet)
    uint8_t spec_id;                    // Specification ID (hardfork version): 0=Frontier, ..., 18=Cancun, 19=Prague
    bool disable_nonce_check;           // Whether to disable nonce checks (useful for testing)
    bool disable_balance_check;         // Whether to disable balance checks (useful for testing)
    bool disable_block_gas_limit;       // Whether to disable block gas limit checks
    bool disable_base_fee;              // Whether to disable base fee checks
    uint32_t max_code_size;             // Maximum contract code size (0 for default 24KB limit)
} RevmConfigFFI;

// Predefined chain configurations
typedef enum {
    ETHEREUM_MAINNET = 0,               // Ethereum mainnet (chain ID 1)
    BSC_MAINNET = 1,                    // BSC mainnet (chain ID 56)
    BSC_TESTNET = 2,                    // BSC testnet Chapel (chain ID 97)
    CUSTOM = 255                        // Custom configuration
} ChainPreset;

// Execution result structure
typedef struct {
    int success;
    unsigned int gas_used;
    unsigned int gas_refunded;
    unsigned char* output_data;
    unsigned int output_len;
    unsigned int logs_count;
    void* logs;  // LogFFI*
    char* created_address;
} ExecutionResultFFI;

// Log structure
typedef struct {
    char* address;
    unsigned int topics_count;
    char** topics;
    unsigned char* data;
    unsigned int data_len;
} LogFFI;

// Deployment result structure
typedef struct {
    int success;
    char* contract_address;
    unsigned int gas_used;
    unsigned int gas_refunded;
} DeploymentResultFFI;

// REVM instance management
RevmInstance* revm_new(void);
RevmInstance* revm_new_with_preset(ChainPreset preset);
RevmInstance* revm_new_with_config(const RevmConfigFFI* config);
void revm_free(RevmInstance* instance);

// Configuration queries
uint64_t revm_get_chain_id(const RevmInstance* instance);
uint8_t revm_get_spec_id(const RevmInstance* instance);

// Account management
int revm_set_balance(RevmInstance* instance, const char* address, const char* balance);
char* revm_get_balance(RevmInstance* instance, const char* address);
int revm_set_nonce(RevmInstance* instance, const char* address, uint64_t nonce);
uint64_t revm_get_nonce(RevmInstance* instance, const char* address);

// Contract deployment
DeploymentResultFFI* revm_deploy_contract(
    RevmInstance* instance,
    const char* from,
    const unsigned char* bytecode,
    unsigned int bytecode_len,
    uint64_t gas_limit
);

// Transaction execution
ExecutionResultFFI* revm_call_contract(
    RevmInstance* instance,
    const char* from,
    const char* to,
    const unsigned char* data,
    unsigned int data_len,
    const char* value,
    uint64_t gas_limit
);

ExecutionResultFFI* revm_view_call_contract(
    RevmInstance* instance,
    const char* from,
    const char* to,
    const unsigned char* data,
    unsigned int data_len,
    uint64_t gas_limit
);

ExecutionResultFFI* revm_transfer(
    RevmInstance* instance,
    const char* from,
    const char* to,
    const char* value,
    uint64_t gas_limit
);

// Storage operations
int revm_set_storage(
    RevmInstance* instance,
    const char* address,
    const char* key,
    const char* value
);

char* revm_get_storage(
    RevmInstance* instance,
    const char* address,
    const char* key
);

// Memory management for results
void revm_free_execution_result(ExecutionResultFFI* result);
void revm_free_deployment_result(DeploymentResultFFI* result);
void revm_free_string(char* str);

// Error handling
const char* revm_get_last_error(RevmInstance* instance);

#ifdef __cplusplus
}
#endif

#endif // REVM_FFI_H 