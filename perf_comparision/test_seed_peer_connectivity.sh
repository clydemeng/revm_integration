#!/bin/bash

# Seed Peer Connectivity Test Script
# Tests if the seed peer is working properly and can serve block data

# Change to the script's directory
cd "$(dirname "$0")"

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}🔍 BSC Seed Peer Connectivity Test${NC}"
echo "=================================="

SEED_PEER_HTTP="http://127.0.0.1:8546"
SEED_PEER_ENODE_FILE="./seed_peer/seed_peer_enode.txt"

# Function to test RPC connectivity
test_rpc_connectivity() {
    echo -e "${YELLOW}📡 Testing RPC connectivity...${NC}"
    
    if curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"web3_clientVersion","params":[],"id":1}' > /dev/null 2>&1; then
        
        VERSION=$(curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"web3_clientVersion","params":[],"id":1}' | \
            python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if 'result' in data:
        print(data['result'])
    else:
        print('Unknown')
except:
    print('Error')
" 2>/dev/null || echo "Error")
        
        echo -e "${GREEN}✅ RPC is accessible${NC}"
        echo -e "${GREEN}   Client: $VERSION${NC}"
        return 0
    else
        echo -e "${RED}❌ RPC not accessible${NC}"
        return 1
    fi
}

# Function to test block data availability
test_block_data() {
    echo -e "${YELLOW}📦 Testing block data availability...${NC}"
    
    # Get current block number
    CURRENT_BLOCK=$(curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
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
    
    if [ "$CURRENT_BLOCK" -gt 0 ]; then
        echo -e "${GREEN}✅ Seed peer has $CURRENT_BLOCK blocks${NC}"
        
        # Test if we can get block details
        BLOCK_HASH=$(curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["0x1", false],"id":1}' | \
            python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if data.get('result') and data['result'].get('hash'):
        print(data['result']['hash'])
    else:
        print('Error')
except:
    print('Error')
" 2>/dev/null || echo "Error")
        
        if [ "$BLOCK_HASH" != "Error" ] && [ -n "$BLOCK_HASH" ]; then
            echo -e "${GREEN}✅ Can retrieve block data (Block 1 hash: $BLOCK_HASH)${NC}"
            return 0
        else
            echo -e "${RED}❌ Cannot retrieve block data${NC}"
            return 1
        fi
    else
        echo -e "${RED}❌ Seed peer has no blocks${NC}"
        return 1
    fi
}

# Function to test peer connectivity
test_peer_info() {
    echo -e "${YELLOW}🔗 Testing peer networking...${NC}"
    
    # Get peer count
    PEER_COUNT=$(curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
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
    
    echo -e "${GREEN}✅ Seed peer connected to $PEER_COUNT peers${NC}"
    
    # Get enode info
    ENODE_INFO=$(curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"admin_nodeInfo","params":[],"id":1}' | \
        python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if data.get('result') and data['result'].get('enode'):
        print(data['result']['enode'])
    else:
        print('Error')
except:
    print('Error')
" 2>/dev/null || echo "Error")
    
    if [ "$ENODE_INFO" != "Error" ] && [ -n "$ENODE_INFO" ]; then
        echo -e "${GREEN}✅ Enode: $ENODE_INFO${NC}"
        
        # Save enode for tests
        echo "$ENODE_INFO" > "$SEED_PEER_ENODE_FILE"
        echo -e "${GREEN}✅ Enode saved to $SEED_PEER_ENODE_FILE${NC}"
        return 0
    else
        echo -e "${RED}❌ Cannot get enode info${NC}"
        return 1
    fi
}

# Function to test static node connection simulation
test_static_node_simulation() {
    echo -e "${YELLOW}🎯 Testing what happens when test node connects to seed peer...${NC}"
    
    # Simulate connecting to the seed peer from another node
    # We'll try to get the same block data a test node would request
    
    echo -e "${BLUE}   → Simulating handshake with seed peer...${NC}"
    
    # Test getting genesis block (what sync starts with)
    GENESIS_HASH=$(curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_getBlockByNumber","params":["0x0", false],"id":1}' | \
        python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if data.get('result') and data['result'].get('hash'):
        print(data['result']['hash'])
    else:
        print('Error')
except:
    print('Error')
" 2>/dev/null || echo "Error")
    
    if [ "$GENESIS_HASH" != "Error" ] && [ -n "$GENESIS_HASH" ]; then
        echo -e "${GREEN}   ✅ Can get genesis block: $GENESIS_HASH${NC}"
        
        # Test getting a few more blocks
        for i in 1 2 3 10 50; do
            BLOCK_HEX=$(printf "0x%x" $i)
            BLOCK_EXISTS=$(curl -s "$SEED_PEER_HTTP" -X POST -H "Content-Type: application/json" \
                -d "{\"jsonrpc\":\"2.0\",\"method\":\"eth_getBlockByNumber\",\"params\":[\"$BLOCK_HEX\", false],\"id\":1}" | \
                python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if data.get('result') and data['result'].get('hash'):
        print('OK')
    else:
        print('MISSING')
except:
    print('ERROR')
" 2>/dev/null || echo "ERROR")
            
            if [ "$BLOCK_EXISTS" = "OK" ]; then
                echo -e "${GREEN}   ✅ Block $i: Available${NC}"
            else
                echo -e "${YELLOW}   ⚠️ Block $i: Not available yet${NC}"
            fi
        done
        
        return 0
    else
        echo -e "${RED}   ❌ Cannot get genesis block${NC}"
        return 1
    fi
}

# Main test execution
echo -e "${BLUE}Running connectivity tests...${NC}"
echo ""

TESTS_PASSED=0
TOTAL_TESTS=4

echo -e "${BLUE}Test 1/4: RPC Connectivity${NC}"
if test_rpc_connectivity; then
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
echo ""

echo -e "${BLUE}Test 2/4: Block Data Availability${NC}"
if test_block_data; then
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
echo ""

echo -e "${BLUE}Test 3/4: Peer Networking${NC}"
if test_peer_info; then
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
echo ""

echo -e "${BLUE}Test 4/4: Static Node Connection Simulation${NC}"
if test_static_node_simulation; then
    TESTS_PASSED=$((TESTS_PASSED + 1))
fi
echo ""

# Summary
echo -e "${BLUE}📊 Test Results Summary${NC}"
echo "======================"
echo -e "${GREEN}Tests passed: $TESTS_PASSED/$TOTAL_TESTS${NC}"

if [ "$TESTS_PASSED" -eq "$TOTAL_TESTS" ]; then
    echo -e "${GREEN}🎉 All tests passed! Seed peer is ready for use.${NC}"
    echo ""
    echo -e "${BLUE}💡 About Static Nodes:${NC}"
    echo "• Static nodes are peers that your node will ALWAYS try to connect to"
    echo "• They're added to static-nodes.json and loaded at startup"
    echo "• Unlike discovered peers, static peers are never dropped"
    echo "• Your seed peer WILL be a static node for test nodes"
    echo "• This ensures instant, reliable connection to your local peer"
    echo ""
    echo -e "${GREEN}✅ Your test node will connect to seed peer as a static node${NC}"
    echo -e "${GREEN}✅ This eliminates peer discovery time completely${NC}"
    echo -e "${GREEN}✅ Sync should start immediately with blocks from seed peer${NC}"
    exit 0
else
    echo -e "${RED}❌ Some tests failed. Check seed peer status.${NC}"
    echo ""
    echo -e "${YELLOW}💡 Troubleshooting:${NC}"
    echo "• Make sure seed peer is running: cd seed_peer && ./run_seed_peer.sh -t"
    echo "• If not running, start it: ./run_seed_peer.sh -s"
    echo "• Check seed peer logs: tail -f seed_peer/logs/seed_peer.log"
    exit 1
fi 