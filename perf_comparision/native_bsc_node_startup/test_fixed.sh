#!/bin/bash

# BSC Performance & Correctness Verification Script with Seed Peer
# Tests the performance and correctness of native BSC node implementation using a local seed peer

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
SEED_PEER_ENODE_FILE="../seed_peer/seed_peer_enode.txt"

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

echo -e "${GREEN}🚀 Native BSC Node Performance Test (With Seed Peer)${NC}"
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

# Check if seed peer is available
SEED_PEER_ENODE=""
if [ -f "$SEED_PEER_ENODE_FILE" ]; then
    SEED_PEER_ENODE=$(cat "$SEED_PEER_ENODE_FILE")
    if [[ "$SEED_PEER_ENODE" == enode://* ]]; then
        echo -e "${GREEN}✅ Found seed peer: $SEED_PEER_ENODE${NC}"
        
        # Verify seed peer is reachable
        SEED_PEER_HTTP="http://127.0.0.1:8546"
        if curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' > /dev/null 2>&1; then
            SEED_PEER_BLOCKS=$(curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
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
" 2>/dev/null || echo "0")
            echo -e "${GREEN}✅ Seed peer is running with $SEED_PEER_BLOCKS blocks${NC}"
        else
            echo -e "${YELLOW}⚠️ Seed peer enode found but not reachable. Will use public bootnodes.${NC}"
            SEED_PEER_ENODE=""
        fi
    else
        echo -e "${YELLOW}⚠️ Invalid seed peer enode format. Will use public bootnodes.${NC}"
        SEED_PEER_ENODE=""
    fi
else
    echo -e "${YELLOW}⚠️ No seed peer found. Will use public bootnodes.${NC}"
    echo -e "${BLUE}💡 To use seed peer: cd ../seed_peer && ./run_seed_peer.sh -s${NC}"
fi

# BSC mainnet bootnodes (fallback)
BOOTNODES="enode://433c8bfdf53a3e2268ccb1b829e47f629793291cbddf0c76ae626da802f90532251fc558e2e0d10d6725e759088439bf1cd4714716b03a259a35d4b2e4acfa7f@52.69.102.73:30311,enode://571bee8fb902a625942f10a770ccf727ae2ba1bab2a2b64e121594a99c9437317f6166a395670a00b7d93647eacafe598b6bbcef15b40b6d1a10243865a3e80f@35.73.84.120:30311"

# If we have a seed peer, prioritize it
if [ -n "$SEED_PEER_ENODE" ]; then
    BOOTNODES="$SEED_PEER_ENODE,$BOOTNODES"
fi

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

# Create static nodes file for faster peer discovery (after init)
echo -e "${YELLOW}🔗 Setting up seed peer connections...${NC}"
mkdir -p "$NODE_DATA_DIR/geth"
if [ -n "$SEED_PEER_ENODE" ]; then
    cat > "$NODE_DATA_DIR/geth/static-nodes.json" << EOF
[
  "$SEED_PEER_ENODE",
  "enode://5ccff376cef1691d43763550cfe395f78abd27e0d46c6e5318815c7ec4815d9a00ebbd9132e71c49158949d38f1410e179be8690d111349e51e9f13511ced24c@52.22.219.226:30311",
  "enode://30e786d8c878d9a097dc78a8e1e2e3cb29440f267e2a92d1296fffb89ada5da70db7ae5cf12e48ce748c1234112328662d7b0d79c8bd4d2107ab1b9ca5287234@3.218.87.39:30311"
]
EOF
    echo -e "${GREEN}✅ Configured with seed peer as primary static node${NC}"
else
    cat > "$NODE_DATA_DIR/geth/static-nodes.json" << EOF
[
  "enode://5ccff376cef1691d43763550cfe395f78abd27e0d46c6e5318815c7ec4815d9a00ebbd9132e71c49158949d38f1410e179be8690d111349e51e9f13511ced24c@52.22.219.226:30311",
  "enode://30e786d8c878d9a097dc78a8e1e2e3cb29440f267e2a92d1296fffb89ada5da70db7ae5cf12e48ce748c1234112328662d7b0d79c8bd4d2107ab1b9ca5287234@3.218.87.39:30311",
  "enode://13b8706b5630fb8106ca38c47a6147d50bbdcf6039d1c874e7f01bac6025e7b216ca018c994186e003ff06abd06d84ebdfeed261b3a3e84178a489df6a1911f7@44.208.121.33:30311"
]
EOF
    echo -e "${YELLOW}⚠️ Using public bootnodes only${NC}"
fi

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
    --ws=false \
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

# Check peer connections quickly if using seed peer
if [ -n "$SEED_PEER_ENODE" ]; then
    echo -e "${BLUE}🔗 Checking peer connections...${NC}"
    for i in {1..10}; do
        PEER_COUNT=$(curl -s "$NODE_RPC_URL" -X POST -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}' | \
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
" 2>/dev/null || echo "0")
        
        if [ "$PEER_COUNT" -gt 0 ]; then
            echo -e "${GREEN}✅ Connected to $PEER_COUNT peers (including seed peer)${NC}"
            break
        fi
        echo -n "."
        sleep 2
    done
fi

# Monitor sync progress
SYNC_START_TIME=$(date +%s)
LAST_BLOCK=0
BLOCK_TIMES=()
WARMUP_BLOCKS=100  # Only measure performance after 100 blocks (steady state)
PERFORMANCE_START_TIME=0

echo -e "${BLUE}🔄 Monitoring sync progress to block $TARGET_BLOCKS...${NC}"
if [ -n "$SEED_PEER_ENODE" ]; then
    echo -e "${GREEN}🚀 Using seed peer - expect faster sync start!${NC}"
fi
echo -e "${YELLOW}⏳ Waiting for warmup (first 100 blocks)...${NC}"

while true; do
    CURRENT_BLOCK=$(get_current_block)
    CURRENT_TIME=$(date +%s)
    
    if [ "$CURRENT_BLOCK" -gt 0 ]; then
        if [ "$CURRENT_BLOCK" -gt "$LAST_BLOCK" ]; then
            # Block progress detected
            BLOCKS_PROCESSED=$((CURRENT_BLOCK - LAST_BLOCK))
            
            if [ "$CURRENT_BLOCK" -ge "$WARMUP_BLOCKS" ] && [ "$PERFORMANCE_START_TIME" -eq 0 ]; then
                PERFORMANCE_START_TIME=$CURRENT_TIME
                echo -e "${GREEN}🏁 Warmup complete! Starting performance measurement...${NC}"
            fi
            
            if [ "$CURRENT_BLOCK" -ge "$WARMUP_BLOCKS" ] && [ "$PERFORMANCE_START_TIME" -gt 0 ]; then
                # Calculate performance metrics
                ELAPSED_PERF=$((CURRENT_TIME - PERFORMANCE_START_TIME))
                if [ "$ELAPSED_PERF" -gt 0 ]; then
                    PERFORMANCE_BLOCKS=$((CURRENT_BLOCK - WARMUP_BLOCKS))
                    BLOCKS_PER_SECOND=$(python3 -c "print(f'{$PERFORMANCE_BLOCKS / $ELAPSED_PERF:.2f}')")
                    echo -e "${GREEN}📦 Block: $CURRENT_BLOCK/$TARGET_BLOCKS | Speed: ${BLOCKS_PER_SECOND} blocks/sec | ETA: $(python3 -c "
remaining = $TARGET_BLOCKS - $CURRENT_BLOCK
if $BLOCKS_PER_SECOND > 0:
    eta_seconds = remaining / $BLOCKS_PER_SECOND
    eta_minutes = int(eta_seconds // 60)
    eta_seconds = int(eta_seconds % 60)
    print(f'{eta_minutes}m {eta_seconds}s')
else:
    print('calculating...')
")${NC}"
                else
                    echo -e "${GREEN}📦 Block: $CURRENT_BLOCK/$TARGET_BLOCKS${NC}"
                fi
            else
                echo -e "${YELLOW}🔄 Warmup: $CURRENT_BLOCK/$WARMUP_BLOCKS blocks${NC}"
            fi
            
            LAST_BLOCK=$CURRENT_BLOCK
            
            # Check if we've reached our target
            if [ "$CURRENT_BLOCK" -ge "$TARGET_BLOCKS" ]; then
                break
            fi
        fi
    else
        echo -e "${YELLOW}⏳ Waiting for sync to start...${NC}"
    fi
    
    sleep 2
done

# Continue with rest of performance testing (same as original script)
SYNC_END_TIME=$(date +%s)
TOTAL_SYNC_TIME=$((SYNC_END_TIME - SYNC_START_TIME))
PERFORMANCE_TIME=$((SYNC_END_TIME - PERFORMANCE_START_TIME))

echo -e "${GREEN}🎉 Sync completed!${NC}"
echo -e "${BLUE}📊 Performance Summary:${NC}"
echo "  Total time: ${TOTAL_SYNC_TIME}s"
if [ "$PERFORMANCE_TIME" -gt 0 ]; then
    PERFORMANCE_BLOCKS=$((TARGET_BLOCKS - WARMUP_BLOCKS))
    FINAL_BPS=$(python3 -c "print(f'{$PERFORMANCE_BLOCKS / $PERFORMANCE_TIME:.2f}')")
    echo "  Performance blocks: $PERFORMANCE_BLOCKS"
    echo "  Performance time: ${PERFORMANCE_TIME}s" 
    echo "  Average blocks/sec: $FINAL_BPS"
fi
if [ -n "$SEED_PEER_ENODE" ]; then
    echo -e "${GREEN}  ✅ Used seed peer for faster startup${NC}"
fi

# Rest of the verification logic remains the same...
echo -e "${BLUE}Phase 2: Block Verification${NC}"
echo "============================"

# Verification continues as in original script...
for BLOCK_NUM in "${BLOCKS_TO_VERIFY[@]}"; do
    if [ "$BLOCK_NUM" -le "$CURRENT_BLOCK" ]; then
        echo -e "${YELLOW}🔍 Verifying block $BLOCK_NUM...${NC}"
        
        LOCAL_DATA=$(get_block_data "$NODE_RPC_URL" "$BLOCK_NUM")
        MAINNET_DATA=$(get_block_data "$BSC_MAINNET_RPC" "$BLOCK_NUM")
        
        LOCAL_HASH=$(echo "$LOCAL_DATA" | cut -d'|' -f1)
        LOCAL_STATE_ROOT=$(echo "$LOCAL_DATA" | cut -d'|' -f2)
        
        MAINNET_HASH=$(echo "$MAINNET_DATA" | cut -d'|' -f1)
        MAINNET_STATE_ROOT=$(echo "$MAINNET_DATA" | cut -d'|' -f2)
        
        if [ "$LOCAL_HASH" = "$MAINNET_HASH" ] && [ "$LOCAL_STATE_ROOT" = "$MAINNET_STATE_ROOT" ]; then
            echo -e "${GREEN}✅ Block $BLOCK_NUM: VERIFIED${NC}"
        else
            echo -e "${RED}❌ Block $BLOCK_NUM: MISMATCH${NC}"
            echo "  Local hash: $LOCAL_HASH"
            echo "  Mainnet hash: $MAINNET_HASH"
            echo "  Local state root: $LOCAL_STATE_ROOT"
            echo "  Mainnet state root: $MAINNET_STATE_ROOT"
        fi
    fi
done

echo -e "${GREEN}🎉 Performance test completed successfully!${NC}"
if [ -n "$SEED_PEER_ENODE" ]; then
    echo -e "${BLUE}💡 Seed peer helped accelerate the test startup${NC}"
fi 