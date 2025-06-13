#!/bin/bash

# BSC Performance & Correctness Verification Script
# Tests the performance and correctness of native BSC node implementation

# Change to the script's directory
cd "$(dirname "$0")"

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
NC='\033[0m' # No Color

# Default geth binary
GETH_BINARY="./geth"

# Parse command-line arguments
while getopts ":b:" opt; do
  case $opt in
    b)
      GETH_BINARY="$OPTARG"
      ;;
    \?)
      echo "Invalid option: -$OPTARG" >&2
      exit 1
      ;;
    :)
      echo "Option -$OPTARG requires an argument." >&2
      exit 1
      ;;
  esac
done

echo -e "${GREEN}🚀 Native BSC Node Performance Test${NC}"
echo "Using binary: $GETH_BINARY"
echo "========================================="

# Configuration
TARGET_BLOCKS=10000
BSC_MAINNET_RPC="https://bsc-dataseed.bnbchain.org"
NODE_RPC_URL="http://127.0.0.1:8545"
NODE_DATA_DIR="./bsc_data"
PERFORMANCE_LOG_DIR="./performance_logs"
RESULTS_FILE="$PERFORMANCE_LOG_DIR/comprehensive_results.json"
NODE_PID_FILE="./node.pid"
BLOCKS_TO_VERIFY=(100 500 1000 10000)

# BSC mainnet bootnodes
BOOTNODES="enode://433c8bfdf53a3e2268ccb1b829e47f629793291cbddf0c76ae626da802f90532251fc558e2e0d10d6725e759088439bf1cd4714716b03a259a35d4b2e4acfa7f@52.69.102.73:30311,enode://571bee8fb902a625942f10a770ccf727ae2ba1bab2a2b64e121594a99c9437317f6166a395670a00b7d93647eacafe598b6bbcef15b40b6d1a10243865a3e80f@35.73.84.120:30311,enode://fac42fb0ba082b7d1eebded216db42161163d42e4f52c9e47716946d64468a62da4ba0b1cac0df5e8bf1e5284861d757339751c33d51dfef318be5168803d0b5@18.203.152.54:30311,enode://3063d1c9e1b824cfbb7c7b6abafa34faec6bb4e7e06941d218d760acdd7963b274278c5c3e63914bd6d1b58504c59ec5522c56f883baceb8538674b92da48a96@34.250.32.100:30311,enode://ad78c64a4ade83692488aa42e4c94084516e555d3f340d9802c2bf106a3df8868bc46eae083d2de4018f40e8d9a9952c32a0943cd68855a9bc9fd07aac982a6d@34.204.214.24:30311,enode://5db798deb67df75d073f8e2953dad283148133acb520625ea804c9c4ad09a35f13592a762d8f89056248f3889f6dcc33490c145774ea4ff2966982294909b37a@107.20.191.97:30311"

# Create static nodes file for faster peer discovery
mkdir -p "$NODE_DATA_DIR/geth"
cat > "$NODE_DATA_DIR/geth/static-nodes.json" << EOF
[
  "enode://5ccff376cef1691d43763550cfe395f78abd27e0d46c6e5318815c7ec4815d9a00ebbd9132e71c49158949d38f1410e179be8690d111349e51e9f13511ced24c@52.22.219.226:30311",
  "enode://30e786d8c878d9a097dc78a8e1e2e3cb29440f267e2a92d1296fffb89ada5da70db7ae5cf12e48ce748c1234112328662d7b0d79c8bd4d2107ab1b9ca5287234@3.218.87.39:30311",
  "enode://13b8706b5630fb8106ca38c47a6147d50bbdcf6039d1c874e7f01bac6025e7b216ca018c994186e003ff06abd06d84ebdfeed261b3a3e84178a489df6a1911f7@44.208.121.33:30311",
  "enode://f56dcbe59ddcf52e2abe5d5f5fded28bf823e7e2fb887cebbfe3c540ed0dfbbd778872e6b0c9c6243fcb79fdf3e1805ae98a7c389091e9cc55bfe6dedfce04b8@3.115.208.145:30311",
  "enode://b4feb14a8247917f25a4603a0a3a58827e6e3954fa1fc0499f3e084476dcb2dc32e444e7c51cecbc1066d2c94062fc16aa80da1a008c94e576b67b84a3a111c5@13.112.103.141:30311",
  "enode://7fed0d5ebfec2d68106cf91d4bbf2c794a22f12a11c18ef643818e8b8a5022f63abccfa50cb34fd30343530f67a70523525d94247b4f8d143dca7524d2ba8630@52.194.28.137:30311"
]
EOF

# Create performance logs directory
mkdir -p "$PERFORMANCE_LOG_DIR"

# Function to check if node is running
check_node_running() {
    if curl -s "$NODE_RPC_URL" -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' > /dev/null 2>&1; then
        return 0
    else
        return 1
    fi
}

# Function to get current block number
get_current_block() {
    curl -s "$NODE_RPC_URL" -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' | \
        python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data:
        print(int(data['result'], 16))
    else:
        print(0)
except:
    print(0)
" 2>/dev/null || echo "0"
}

# Function to get block data
get_block_data() {
    local rpc_url=$1
    local block_number=$2
    local block_hex=$(printf "0x%x" $block_number)
    
    curl -s "$rpc_url" -X POST -H "Content-Type: application/json" \
        -d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_getBlockByNumber\",\"params\":[\"$block_hex\", false],\"id\":1}" | \
        python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if data.get('result'):
        result = data['result']
        print(f\"{result.get('hash', 'N/A')}|{result.get('stateRoot', 'N/A')}|{result.get('parentHash', 'N/A')}\")
    else:
        print('ERROR|ERROR|ERROR')
except:
    print('ERROR|ERROR|ERROR')
" 2>/dev/null || echo "ERROR|ERROR|ERROR"
}

# Function to stop node
stop_node() {
    if [ -f "$NODE_PID_FILE" ]; then
        NODE_PID=$(cat "$NODE_PID_FILE")
        if kill -0 "$NODE_PID" 2>/dev/null; then
            echo -e "${YELLOW}🛑 Stopping BSC node (PID: $NODE_PID)...${NC}"
            kill "$NODE_PID"
            rm "$NODE_PID_FILE"
        fi
    fi
}

# Trap to ensure cleanup on exit
trap 'stop_node; exit' INT TERM EXIT

echo -e "${BLUE}Phase 1: Performance Testing${NC}"
echo "============================="

# Clean data directory for fresh sync
if [ -d "$NODE_DATA_DIR" ]; then
    echo -e "${YELLOW}🗑️ Cleaning previous node data...${NC}"
    rm -rf "$NODE_DATA_DIR"
fi

# Initialize node with genesis
echo -e "${YELLOW}🏗️ Initializing fresh node...${NC}"
"$GETH_BINARY" --datadir "$NODE_DATA_DIR" init ./genesis.json

echo -e "${YELLOW}🚀 Starting BSC node with full sync mode...${NC}"

# Start node with improved peer discovery
nohup "$GETH_BINARY" \
    --config ./configs/performance_config.toml \
    --datadir "$NODE_DATA_DIR" \
    --syncmode full \
    --http \
    --http.addr "127.0.0.1" \
    --http.port 8545 \
    --http.api "eth,net,web3,debug" \
    --ipcdisable \
    --metrics \
    --metrics.addr "127.0.0.1" \
    --metrics.port 6060 \
    --verbosity 3 \
    --log.file "./performance_logs/node.log" \
    --maxpeers 40 \
    --port 30303 \
    --bootnodes "$BOOTNODES" \
    > "$PERFORMANCE_LOG_DIR/startup.log" 2>&1 &

echo $! > "$NODE_PID_FILE"

# Wait for node to be ready
echo -e "${YELLOW}⏳ Waiting for node to be ready for sync...${NC}"
for i in {1..30}; do
    if check_node_running; then
        echo -e "${GREEN}✅ Node is ready and starting sync!${NC}"
        break
    fi
    echo -n "."
    sleep 2
done

if ! check_node_running; then
    echo -e "${RED}❌ Failed to start node${NC}"
    exit 1
fi

# Monitor sync progress
SYNC_START_TIME=$(date +%s)
LAST_BLOCK=0
BLOCK_TIMES=()
WARMUP_BLOCKS=100  # Only measure performance after 100 blocks (steady state)
PERFORMANCE_START_TIME=0

echo -e "${BLUE}🔄 Monitoring sync progress to block $TARGET_BLOCKS...${NC}"
echo -e "${YELLOW}⏳ Waiting for warmup (first 100 blocks)...${NC}"

while true; do
    CURRENT_BLOCK=$(get_current_block)
    CURRENT_TIME=$(date +%s)
    
    if [ "$CURRENT_BLOCK" -gt "$LAST_BLOCK" ]; then
        if [ "$LAST_BLOCK" -gt 0 ]; then
            TIME_DIFF=$((CURRENT_TIME - LAST_BLOCK_TIME))
            if [ "$TIME_DIFF" -gt 0 ]; then
                BLOCKS_PROCESSED=$((CURRENT_BLOCK - LAST_BLOCK))
                AVG_TIME_PER_BLOCK=$((TIME_DIFF * 1000 / BLOCKS_PROCESSED))
                
                # Only include measurements after warmup period
                if [ "$CURRENT_BLOCK" -gt "$WARMUP_BLOCKS" ]; then
                    if [ "$PERFORMANCE_START_TIME" -eq 0 ]; then
                        PERFORMANCE_START_TIME=$CURRENT_TIME
                        echo -e "${GREEN}🚀 Warmup complete! Now measuring steady-state performance...${NC}"
                    fi
                    BLOCK_TIMES+=($AVG_TIME_PER_BLOCK)
                    echo -e "${GREEN}📊 Block $CURRENT_BLOCK: ~${AVG_TIME_PER_BLOCK}ms per block (processed $BLOCKS_PROCESSED blocks in ${TIME_DIFF}s)${NC}"
                else
                    echo -e "${YELLOW}🔥 Warmup Block $CURRENT_BLOCK: ~${AVG_TIME_PER_BLOCK}ms per block${NC}"
                fi
            fi
        fi
        
        LAST_BLOCK=$CURRENT_BLOCK
        LAST_BLOCK_TIME=$CURRENT_TIME
    fi
    
    if [ "$CURRENT_BLOCK" -ge "$TARGET_BLOCKS" ]; then
        echo -e "${GREEN}🎉 Reached target block $TARGET_BLOCKS!${NC}"
        break
    fi
    
    # Status update every 20 seconds (faster updates)
    if [ $((CURRENT_TIME % 20)) -eq 0 ]; then
        echo -e "${YELLOW}⏳ Current block: $CURRENT_BLOCK/$TARGET_BLOCKS${NC}"
    fi
    
    sleep 3  # Check more frequently
done

# Calculate performance metrics
END_TIME=$(date +%s)
TOTAL_SYNC_TIME=$((END_TIME - SYNC_START_TIME))
BLOCKS_SYNCED=$CURRENT_BLOCK

# Calculate steady-state performance metrics
if [ ${#BLOCK_TIMES[@]} -gt 0 ]; then
    TOTAL_TIME=0
    for time in "${BLOCK_TIMES[@]}"; do
        TOTAL_TIME=$((TOTAL_TIME + time))
    done
    AVG_BLOCK_TIME=$((TOTAL_TIME / ${#BLOCK_TIMES[@]}))
    STEADY_STATE_TIME=$((END_TIME - PERFORMANCE_START_TIME))
    STEADY_STATE_BLOCKS=$((BLOCKS_SYNCED - WARMUP_BLOCKS))
else
    AVG_BLOCK_TIME=0
    STEADY_STATE_TIME=$TOTAL_SYNC_TIME
    STEADY_STATE_BLOCKS=$BLOCKS_SYNCED
fi

echo ""
echo -e "${BLUE}Phase 2: Correctness Verification${NC}"
echo "=================================="

# Verification results
VERIFICATION_RESULTS=()
ALL_CORRECT=true
BLOCKS_VERIFIED=0
BLOCKS_CORRECT=0

for block_num in "${BLOCKS_TO_VERIFY[@]}"; do
    BLOCKS_VERIFIED=$((BLOCKS_VERIFIED + 1))
    echo -e "${YELLOW}📊 Checking Block $block_num:${NC}"
    
    # Get mainnet data
    echo -n "  🌐 BSC Mainnet: "
    MAINNET_DATA=$(get_block_data "$BSC_MAINNET_RPC" $block_num)
    IFS='|' read -r MAINNET_HASH MAINNET_STATE MAINNET_PARENT <<< "$MAINNET_DATA"
    
    if [ "$MAINNET_HASH" = "ERROR" ]; then
        echo -e "${RED}❌ Failed to get mainnet data${NC}"
        ALL_CORRECT=false
        VERIFICATION_RESULTS+=("Block $block_num: ❌ FAILED")
        continue
    fi
    
    echo -e "${GREEN}✅${NC}"
    echo "    Hash: $MAINNET_HASH"
    
    # Get local data
    echo -n "  💻 Local Node:  "
    LOCAL_DATA=$(get_block_data "$NODE_RPC_URL" $block_num)
    IFS='|' read -r LOCAL_HASH LOCAL_STATE LOCAL_PARENT <<< "$LOCAL_DATA"
    
    if [ "$LOCAL_HASH" = "ERROR" ]; then
        echo -e "${RED}❌ Failed to get local data${NC}"
        ALL_CORRECT=false
        VERIFICATION_RESULTS+=("Block $block_num: ❌ FAILED")
        continue
    fi
    
    echo -e "${GREEN}✅${NC}"
    echo "    Hash: $LOCAL_HASH"
    
    # Compare data
    echo -n "  🔍 Comparison:  "
    if [ "$MAINNET_HASH" = "$LOCAL_HASH" ] && [ "$MAINNET_STATE" = "$LOCAL_STATE" ]; then
        echo -e "${GREEN}✅ PERFECT MATCH${NC}"
        VERIFICATION_RESULTS+=("Block $block_num: ✅ CORRECT")
        BLOCKS_CORRECT=$((BLOCKS_CORRECT + 1))
    else
        echo -e "${RED}❌ MISMATCH${NC}"
        VERIFICATION_RESULTS+=("Block $block_num: ❌ INCORRECT")
        ALL_CORRECT=false
    fi
    echo ""
done

# Calculate metrics
CORRECTNESS_PERCENTAGE=$((BLOCKS_CORRECT * 100 / BLOCKS_VERIFIED))
BLOCKS_PER_SECOND=$(echo "scale=2; $BLOCKS_SYNCED / $TOTAL_SYNC_TIME" | bc -l 2>/dev/null || echo "0")

# Calculate time for exactly 1000 blocks
TIME_FOR_1000_BLOCKS=$(python3 -c "print(f'{1000 * ($TOTAL_SYNC_TIME / $BLOCKS_SYNCED):.1f}')")

# Generate comprehensive results
cat > "$RESULTS_FILE" << EOF
{
    "test_info": {
        "test_type": "comprehensive_bsc_performance_verification",
        "timestamp": "$(date -Iseconds)",
        "target_blocks": $TARGET_BLOCKS,
        "node_version": "BSC-geth",
        "sync_mode": "full"
    },
    "performance_results": {
        "blocks_synced": $BLOCKS_SYNCED,
        "total_sync_time_seconds": $TOTAL_SYNC_TIME,
        "time_for_1000_blocks_seconds": $TIME_FOR_1000_BLOCKS,
        "average_block_time_ms": $AVG_BLOCK_TIME,
        "blocks_per_second": $(echo "scale=2; $BLOCKS_SYNCED / $TOTAL_SYNC_TIME" | bc -l 2>/dev/null || echo "0"),
        "throughput_1000_blocks": $(echo "scale=2; 1000 / $TIME_FOR_1000_BLOCKS" | bc -l 2>/dev/null || echo "0")
    },
    "correctness_verification": {
        "all_blocks_correct": $([ "$ALL_CORRECT" = true ] && echo "true" || echo "false"),
        "verified_blocks": [$(printf '%s\n' "${BLOCKS_TO_VERIFY[@]}" | paste -sd, -)],
        "verification_summary": "$([ "$ALL_CORRECT" = true ] && echo "All verified blocks match BSC mainnet exactly" || echo "Some blocks failed verification")"
    },
    "revm_projections": {
        "expected_improvement": "50%",
        "projected_time_1000_blocks_seconds": $(echo "scale=1; $TIME_FOR_1000_BLOCKS * 0.5" | bc -l 2>/dev/null || echo "0"),
        "projected_avg_block_time_ms": $(echo "scale=1; $AVG_BLOCK_TIME * 0.5" | bc -l 2>/dev/null || echo "0"),
        "projected_throughput": $(echo "scale=1; 1000 / ($TIME_FOR_1000_BLOCKS * 0.5)" | bc -l 2>/dev/null || echo "0")
    }
}
EOF

echo ""
echo -e "${GREEN}🎉 TEST COMPLETED!${NC}"
echo "=================================="
echo -e "${BLUE}📊 Performance Results:${NC}"
echo "  • Total blocks synced: $CURRENT_BLOCK"
echo "  • Steady-state blocks: $((CURRENT_BLOCK - WARMUP_BLOCKS))"
echo "  • Steady-state time: ${STEADY_STATE_TIME}s"
echo "  • Average block time: ${AVG_BLOCK_TIME}ms"
echo "  • Throughput: $BLOCKS_PER_SECOND blocks/second"

echo -e "\n${BLUE}🔍 Correctness Verification:${NC}"
for result in "${VERIFICATION_RESULTS[@]}"; do
    echo "  • $result"
done

echo -e "\n${GREEN}✅ NATIVE BSC STATUS:${NC}"
echo "• Performance: $BLOCKS_PER_SECOND blocks/second"
echo "• Correctness: $CORRECTNESS_PERCENTAGE% verified"

echo -e "\n📁 Results saved to: $RESULTS_FILE"

# Cleanup
stop_node 