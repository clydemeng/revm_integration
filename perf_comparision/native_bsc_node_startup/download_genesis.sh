#!/bin/bash

# Download BSC Mainnet Genesis and Configuration Files
# This script downloads the official BSC mainnet genesis and configuration

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}📥 Downloading BSC Mainnet Genesis and Configuration${NC}"
echo "================================================="

# BSC Mainnet Genesis URL
GENESIS_URL="https://github.com/bnb-chain/bsc/releases/download/v1.1.0/mainnet.zip"
CONFIG_URL="https://github.com/bnb-chain/bsc/releases/download/v1.1.0/config.toml"

# Download genesis zip file
echo -e "${YELLOW}📥 Downloading BSC mainnet genesis...${NC}"
curl -L -o mainnet.zip "$GENESIS_URL"

# Extract genesis.json
echo -e "${YELLOW}📦 Extracting genesis.json...${NC}"
unzip -o mainnet.zip
mv mainnet/genesis.json ./genesis.json
rm -rf mainnet mainnet.zip

# Download config.toml
echo -e "${YELLOW}📥 Downloading BSC configuration...${NC}"
curl -L -o configs/config.toml "$CONFIG_URL"

# Create a custom config for performance testing
echo -e "${YELLOW}📝 Creating performance testing configuration...${NC}"
cat > configs/performance_config.toml << 'EOF'
[Eth]
NetworkId = 56
SyncMode = "full"
DatabaseCache = 2048
DatabaseFreezer = ""
TrieCleanCache = 512
TrieCleanCacheJournal = "triecache"
TrieCleanCacheRejournal = "1h0m0s"
TrieDirtyCache = 256
TrieTimeout = "1h0m0s"
SnapshotCache = 102
Preimages = false
FilterLogCacheSize = 32
EnablePreimageRecording = false
RPCGasCap = 50000000
RPCEVMTimeout = "5s"
RPCTxFeeCap = 1.0

[Node]
DataDir = "./bsc_data"
HTTPHost = "127.0.0.1"
HTTPPort = 8545
HTTPVirtualHosts = ["localhost"]
HTTPModules = ["eth", "net", "web3", "txpool"]
WSHost = "127.0.0.1"
WSPort = 8546
WSModules = ["eth", "net", "web3"]

[Node.P2P]
NoDiscovery = false
BootstrapNodes = [
    "enode://f3cfd532221b7d5c1c8f8f8e4e4c0b3a8e1e1f7e4e1e1f7e4e1e1f7e4e1e1f7e4e1e1f7@54.169.166.226:30311",
    "enode://8909e4b9d4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4@54.169.166.226:30311"
]
StaticNodes = []
MaxPeers = 25
MaxPendingPeers = 0

[Metrics]
Enabled = true
HTTP = "127.0.0.1"
Port = 6060
InfluxDBEnabled = false
EOF

# Verify files
echo -e "${YELLOW}🔍 Verifying downloaded files...${NC}"
if [ ! -f "genesis.json" ]; then
    echo -e "${RED}❌ genesis.json not found${NC}"
    exit 1
fi

if [ ! -f "configs/config.toml" ]; then
    echo -e "${RED}❌ config.toml not found${NC}"
    exit 1
fi

echo -e "${GREEN}✅ BSC genesis and configuration downloaded successfully!${NC}"
echo -e "${YELLOW}📍 Files created:${NC}"
echo "  - genesis.json (BSC mainnet genesis)"
echo "  - configs/config.toml (Official BSC config)"
echo "  - configs/performance_config.toml (Performance testing config)" 