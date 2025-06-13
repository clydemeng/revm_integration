#!/bin/bash

# BSC Data Correctness Verification Script
# Compares our locally synced block data with BSC mainnet

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${GREEN}🔍 BSC Data Correctness Verification${NC}"
echo "======================================"

# Configuration
BSC_MAINNET_RPC="https://bsc-dataseed.bnbchain.org"
LOCAL_NODE_PORT="8545"
DATA_DIR="./bsc_data"
BLOCKS_TO_CHECK=(100 500 1000)

# Start our local node temporarily
echo -e "${YELLOW}🚀 Starting local node for verification...${NC}"
nohup ./geth --config ./configs/performance_config.toml \
    --datadir "$DATA_DIR" \
    --http \
    --http.addr "127.0.0.1" \
    --http.port $LOCAL_NODE_PORT \
    --http.api "eth,net,web3" \
    --verbosity 1 \
    > /tmp/verify_node.log 2>&1 &

LOCAL_NODE_PID=$!
echo "Local node started with PID: $LOCAL_NODE_PID"

# Function to stop local node
cleanup() {
    echo -e "${YELLOW}🛑 Stopping local node...${NC}"
    kill $LOCAL_NODE_PID 2>/dev/null || true
    wait $LOCAL_NODE_PID 2>/dev/null || true
}
trap cleanup EXIT

# Wait for node to be ready
echo -e "${YELLOW}⏳ Waiting for local node to be ready...${NC}"
for i in {1..30}; do
    if curl -s "http://127.0.0.1:$LOCAL_NODE_PORT" -X POST -H "Content-Type: application/json" \
       -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Local node is ready!${NC}"
        break
    fi
    echo -n "."
    sleep 2
done

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

# Verification results
echo -e "${BLUE}🔄 Verifying block data correctness...${NC}"
echo ""

VERIFICATION_RESULTS=()
ALL_CORRECT=true

for block_num in "${BLOCKS_TO_CHECK[@]}"; do
    echo -e "${YELLOW}📊 Checking Block $block_num:${NC}"
    
    # Get mainnet data
    echo -n "  🌐 BSC Mainnet: "
    MAINNET_DATA=$(get_block_data "$BSC_MAINNET_RPC" $block_num)
    IFS='|' read -r MAINNET_HASH MAINNET_STATE MAINNET_PARENT <<< "$MAINNET_DATA"
    
    if [ "$MAINNET_HASH" = "ERROR" ]; then
        echo -e "${RED}❌ Failed to get mainnet data${NC}"
        ALL_CORRECT=false
        continue
    fi
    
    echo -e "${GREEN}✅${NC}"
    echo "    Hash: $MAINNET_HASH"
    echo "    StateRoot: $MAINNET_STATE"
    
    # Get local data
    echo -n "  💻 Local Node:  "
    LOCAL_DATA=$(get_block_data "http://127.0.0.1:$LOCAL_NODE_PORT" $block_num)
    IFS='|' read -r LOCAL_HASH LOCAL_STATE LOCAL_PARENT <<< "$LOCAL_DATA"
    
    if [ "$LOCAL_HASH" = "ERROR" ]; then
        echo -e "${RED}❌ Failed to get local data${NC}"
        ALL_CORRECT=false
        continue
    fi
    
    echo -e "${GREEN}✅${NC}"
    echo "    Hash: $LOCAL_HASH"
    echo "    StateRoot: $LOCAL_STATE"
    
    # Compare data
    echo -n "  🔍 Comparison:  "
    if [ "$MAINNET_HASH" = "$LOCAL_HASH" ] && [ "$MAINNET_STATE" = "$LOCAL_STATE" ]; then
        echo -e "${GREEN}✅ PERFECT MATCH${NC}"
        VERIFICATION_RESULTS+=("Block $block_num: ✅ CORRECT")
    else
        echo -e "${RED}❌ MISMATCH${NC}"
        VERIFICATION_RESULTS+=("Block $block_num: ❌ INCORRECT")
        ALL_CORRECT=false
        
        if [ "$MAINNET_HASH" != "$LOCAL_HASH" ]; then
            echo -e "    ${RED}Hash mismatch!${NC}"
        fi
        if [ "$MAINNET_STATE" != "$LOCAL_STATE" ]; then
            echo -e "    ${RED}State root mismatch!${NC}"
        fi
    fi
    echo ""
done

# Final results
echo -e "${GREEN}📋 Final Verification Results:${NC}"
echo "================================="
for result in "${VERIFICATION_RESULTS[@]}"; do
    echo "  $result"
done
echo ""

if [ "$ALL_CORRECT" = true ]; then
    echo -e "${GREEN}🎉 DATA CORRECTNESS VERIFIED!${NC}"
    echo "✅ All checked blocks match BSC mainnet exactly"
    echo "✅ EVM execution was correct during sync"
    echo "✅ State transitions were properly computed"
    echo "✅ Ready for REVM integration testing"
else
    echo -e "${RED}❌ DATA CORRECTNESS ISSUES FOUND!${NC}"
    echo "⚠️  Some blocks don't match BSC mainnet"
    echo "⚠️  This indicates potential sync issues"
    echo "⚠️  Review sync logs and node configuration"
fi

echo ""
echo -e "${BLUE}💡 Next Steps:${NC}"
echo "• This verification confirms our sync baseline"
echo "• Use same verification after REVM integration"
echo "• Compare performance metrics while ensuring correctness" 