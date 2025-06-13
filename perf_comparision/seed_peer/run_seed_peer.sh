#!/bin/bash

# BSC Seed Peer Node Script
# Runs a dedicated BSC node that syncs to target blocks and serves as a reliable peer

# Change to the script's directory
cd "$(dirname "$0")"

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
GETH_BINARY="./geth_seed_peer"
NODE_DATA_DIR="./seed_peer_data"
SEED_PEER_PORT=30304
SEED_PEER_HTTP_PORT=8546
SEED_PEER_PID_FILE="./seed_peer.pid"
TARGET_SYNC_BLOCKS=50   # Reduced for faster initial sync with proven static nodes
MAX_PEERS=25
GENESIS_FILE="../native_bsc_node_startup/genesis.json"
CONFIG_FILE="../native_bsc_node_startup/configs/performance_config.toml"

# BSC mainnet bootnodes - using proven working configuration from bsc_performance_verification.sh
BOOTNODES="enode://433c8bfdf53a3e2268ccb1b829e47f629793291cbddf0c76ae626da802f90532251fc558e2e0d10d6725e759088439bf1cd4714716b03a259a35d4b2e4acfa7f@52.69.102.73:30311,enode://571bee8fb902a625942f10a770ccf727ae2ba1bab2a2b64e121594a99c9437317f6166a395670a00b7d93647eacafe598b6bbcef15b40b6d1a10243865a3e80f@35.73.84.120:30311,enode://fac42fb0ba082b7d1eebded216db42161163d42e4f52c9e47716946d64468a62da4ba0b1cac0df5e8bf1e5284861d757339751c33d51dfef318be5168803d0b5@18.203.152.54:30311,enode://3063d1c9e1b824cfbb7c7b6abafa34faec6bb4e7e06941d218d760acdd7963b274278c5c3e63914bd6d1b58504c59ec5522c56f883baceb8538674b92da48a96@34.250.32.100:30311,enode://ad78c64a4ade83692488aa42e4c94084516e555d3f340d9802c2bf106a3df8868bc46eae083d2de4018f40e8d9a9952c32a0943cd68855a9bc9fd07aac982a6d@34.204.214.24:30311,enode://5db798deb67df75d073f8e2953dad283148133acb520625ea804c9c4ad09a35f13592a762d8f89056248f3889f6dcc33490c145774ea4ff2966982294909b37a@107.20.191.97:30311"

# Parse command-line arguments
while getopts ":sktrh" opt; do
  case $opt in
    s)
      echo "Starting seed peer..."
      ACTION="start"
      ;;
    k)
      echo "Stopping seed peer..."
      ACTION="stop"
      ;;
    t)
      echo "Getting seed peer enode info..."
      ACTION="info"
      ;;
    r)
      echo "Restarting seed peer..."
      ACTION="restart"
      ;;
    h)
      echo "BSC Seed Peer Management Script"
      echo "Usage: $0 [OPTIONS]"
      echo "  -s    Start seed peer node"
      echo "  -k    Stop seed peer node"
      echo "  -t    Get seed peer enode info"
      echo "  -r    Restart seed peer node"
      echo "  -h    Show this help"
      exit 0
      ;;
    \?)
      echo "Invalid option: -$OPTARG" >&2
      echo "Use -h for help"
      exit 1
      ;;
  esac
done

# Default to start if no option provided
if [ -z "$ACTION" ]; then
    ACTION="start"
fi

# Function to check if seed peer is running
check_seed_peer_running() {
    if [ -f "$SEED_PEER_PID_FILE" ]; then
        local pid=$(cat "$SEED_PEER_PID_FILE")
        if kill -0 "$pid" 2>/dev/null; then
            return 0
        else
            rm -f "$SEED_PEER_PID_FILE"
            return 1
        fi
    fi
    return 1
}

# Function to get current block number
get_current_block() {
    curl -s "http://127.0.0.1:$SEED_PEER_HTTP_PORT" -X POST -H "Content-Type: application/json" \
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

# Function to get enode info
get_enode_info() {
    curl -s "http://127.0.0.1:$SEED_PEER_HTTP_PORT" -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"admin_nodeInfo","params":[],"id":1}' | \
        python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    if data.get('result') and data['result'].get('enode'):
        print(data['result']['enode'])
    else:
        print('ERROR: Could not get enode info')
except:
    print('ERROR: Failed to parse enode response')
" 2>/dev/null || echo "ERROR: Failed to get enode"
}

# Function to stop seed peer
stop_seed_peer() {
    if check_seed_peer_running; then
        local pid=$(cat "$SEED_PEER_PID_FILE")
        echo -e "${YELLOW}🛑 Stopping seed peer (PID: $pid)...${NC}"
        kill "$pid"
        
        # Wait for graceful shutdown
        for i in {1..10}; do
            if ! kill -0 "$pid" 2>/dev/null; then
                break
            fi
            sleep 1
        done
        
        # Force kill if still running
        if kill -0 "$pid" 2>/dev/null; then
            echo -e "${YELLOW}💀 Force killing seed peer...${NC}"
            kill -9 "$pid"
        fi
        
        rm -f "$SEED_PEER_PID_FILE"
        echo -e "${GREEN}✅ Seed peer stopped${NC}"
    else
        echo -e "${YELLOW}⚠️ Seed peer is not running${NC}"
    fi
}

# Function to start seed peer
start_seed_peer() {
    if check_seed_peer_running; then
        echo -e "${YELLOW}⚠️ Seed peer is already running (PID: $(cat $SEED_PEER_PID_FILE))${NC}"
        return 0
    fi
    
    echo -e "${GREEN}🚀 Starting BSC Seed Peer Node${NC}"
    echo "Target blocks: $TARGET_SYNC_BLOCKS"
    echo "Port: $SEED_PEER_PORT"
    echo "HTTP RPC Port: $SEED_PEER_HTTP_PORT"
    echo "=================================="
    
    # Create data directory
    mkdir -p "$NODE_DATA_DIR/geth"
    mkdir -p "./logs"
    
    # Initialize if not already done
    if [ ! -f "$NODE_DATA_DIR/geth/chaindata/CURRENT" ]; then
        echo -e "${YELLOW}🏗️ Initializing seed peer node...${NC}"
        "$GETH_BINARY" --datadir "$NODE_DATA_DIR" init "$GENESIS_FILE"
    fi
    
    # Create static nodes for seed peer - using exact working configuration from bsc_performance_verification.sh
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
    
    # Start the seed peer node
    echo -e "${YELLOW}🚀 Starting seed peer with sync target of $TARGET_SYNC_BLOCKS blocks...${NC}"
    
    nohup "$GETH_BINARY" \
        --config "$CONFIG_FILE" \
        --datadir "$NODE_DATA_DIR" \
        --syncmode full \
        --http \
        --http.addr "127.0.0.1" \
        --http.port "$SEED_PEER_HTTP_PORT" \
        --http.api "eth,net,web3,debug,admin" \
        --http.corsdomain "*" \
        --ws \
        --ws.addr "127.0.0.1" \
        --ws.port $((SEED_PEER_HTTP_PORT + 1000)) \
        --ws.api "eth,net,web3" \
        --ipcdisable \
        --port "$SEED_PEER_PORT" \
        --maxpeers "$MAX_PEERS" \
        --bootnodes "$BOOTNODES" \
        --verbosity 3 \
        --log.file "./logs/seed_peer.log" \
        --metrics \
        --metrics.addr "127.0.0.1" \
        --metrics.port $((SEED_PEER_HTTP_PORT + 500)) \
        > "./logs/seed_peer_startup.log" 2>&1 &
    
    echo $! > "$SEED_PEER_PID_FILE"
    
    # Wait for node to be ready
    echo -e "${YELLOW}⏳ Waiting for seed peer to be ready...${NC}"
    for i in {1..30}; do
        if curl -s "http://127.0.0.1:$SEED_PEER_HTTP_PORT" -X POST -H "Content-Type: application/json" \
            -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' > /dev/null 2>&1; then
            echo -e "${GREEN}✅ Seed peer is ready!${NC}"
            break
        fi
        echo -n "."
        sleep 2
    done
    
    if ! curl -s "http://127.0.0.1:$SEED_PEER_HTTP_PORT" -X POST -H "Content-Type: application/json" \
        -d '{"jsonrpc":"2.0","method":"eth_blockNumber","params":[],"id":1}' > /dev/null 2>&1; then
        echo -e "${RED}❌ Failed to start seed peer${NC}"
        stop_seed_peer
        exit 1
    fi
    
    # Monitor sync progress
    echo -e "${BLUE}🔄 Monitoring sync progress...${NC}"
    echo "Will stop syncing after reaching block $TARGET_SYNC_BLOCKS"
    
    LAST_BLOCK=0
    STALL_COUNT=0
    MAX_STALL_COUNT=10
    
    while true; do
        CURRENT_BLOCK=$(get_current_block)
        
        if [ "$CURRENT_BLOCK" -gt 0 ]; then
            if [ "$CURRENT_BLOCK" -gt "$LAST_BLOCK" ]; then
                echo -e "${GREEN}📦 Block: $CURRENT_BLOCK/${TARGET_SYNC_BLOCKS}${NC}"
                LAST_BLOCK=$CURRENT_BLOCK
                STALL_COUNT=0
                
                # Check if we've reached our target
                if [ "$CURRENT_BLOCK" -ge "$TARGET_SYNC_BLOCKS" ]; then
                    echo -e "${GREEN}🎯 Reached target block $TARGET_SYNC_BLOCKS!${NC}"
                    break
                fi
            else
                STALL_COUNT=$((STALL_COUNT + 1))
                if [ "$STALL_COUNT" -ge "$MAX_STALL_COUNT" ]; then
                    echo -e "${YELLOW}⚠️ Sync appears stalled at block $CURRENT_BLOCK${NC}"
                    echo -e "${YELLOW}Will continue serving as peer node...${NC}"
                    break
                fi
            fi
        fi
        
        sleep 5
    done
    
    # Get enode info for tests to use
    echo -e "${BLUE}📋 Seed Peer Information:${NC}"
    ENODE_INFO=$(get_enode_info)
    echo -e "${GREEN}Enode: $ENODE_INFO${NC}"
    echo -e "${GREEN}HTTP RPC: http://127.0.0.1:$SEED_PEER_HTTP_PORT${NC}"
    echo -e "${GREEN}P2P Port: $SEED_PEER_PORT${NC}"
    echo -e "${GREEN}Current Block: $(get_current_block)${NC}"
    
    # Save enode info for tests
    echo "$ENODE_INFO" > "./seed_peer_enode.txt"
    
    echo -e "${GREEN}🎉 Seed peer is ready to serve as a peer for your tests!${NC}"
    echo -e "${YELLOW}💡 Use the enode above in your test configurations${NC}"
}

# Function to show seed peer info
show_seed_peer_info() {
    if ! check_seed_peer_running; then
        echo -e "${YELLOW}⚠️ Seed peer is not running${NC}"
        exit 1
    fi
    
    echo -e "${BLUE}📋 Seed Peer Status:${NC}"
    echo -e "${GREEN}PID: $(cat $SEED_PEER_PID_FILE)${NC}"
    echo -e "${GREEN}Current Block: $(get_current_block)${NC}"
    echo -e "${GREEN}HTTP RPC: http://127.0.0.1:$SEED_PEER_HTTP_PORT${NC}"
    echo -e "${GREEN}P2P Port: $SEED_PEER_PORT${NC}"
    
    ENODE_INFO=$(get_enode_info)
    echo -e "${GREEN}Enode: $ENODE_INFO${NC}"
    
    # Save enode info
    echo "$ENODE_INFO" > "./seed_peer_enode.txt"
}

# Execute action
case $ACTION in
    start)
        start_seed_peer
        ;;
    stop)
        stop_seed_peer
        ;;
    info)
        show_seed_peer_info
        ;;
    restart)
        stop_seed_peer
        sleep 2
        start_seed_peer
        ;;
    *)
        echo "Unknown action: $ACTION"
        exit 1
        ;;
esac 