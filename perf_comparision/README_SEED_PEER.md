# BSC Seed Peer Optimization Guide

This guide explains how to use a dedicated BSC seed peer to dramatically speed up your BSC node performance tests by eliminating the lengthy peer discovery process.

## Overview

The seed peer approach solves the problem of waiting minutes for peer discovery by:
1. Running a dedicated BSC node that syncs to 1500+ blocks
2. Serving as a reliable local peer for your test nodes
3. Enabling instant peer connections for faster test startup

## Architecture

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Seed Peer     │    │   Native Test    │    │   REVM Test     │
│   (Port 30304)  │◄──►│   (Port 30303)   │    │   (Port 30303)  │
│   RPC: 8546     │    │   RPC: 8545      │    │   RPC: 8545     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

## Quick Start

### Step 1: Start the Seed Peer

First, ensure your BSC geth binary is built and ready:

```bash
# Build BSC geth if not already done
cd ../../..  # Go to forked_bsc root
make geth

# Start the seed peer
cd revm_integration/perf_comparision/seed_peer
./run_seed_peer.sh -s
```

The seed peer will:
- Sync to 1500 blocks (configurable)
- Run on port 30304 (HTTP RPC on 8546)  
- Save its enode information for test scripts
- Continue running as a peer node

### Step 2: Run Your Tests

Once the seed peer is running, use the optimized test scripts:

**For Native BSC Testing:**
```bash
cd ../native_bsc_node_startup
./bsc_performance_verification_with_seed_peer.sh
```

**For REVM BSC Testing:**
```bash
cd ../revm_bsc_node_startup  
./revmffi_bsc_perf_verification_with_seed_peer.sh
```

Both scripts will automatically:
- Detect the running seed peer
- Connect to it immediately
- Start syncing much faster than with public peer discovery

## Seed Peer Management

### Commands

```bash
# Start seed peer
./run_seed_peer.sh -s

# Stop seed peer  
./run_seed_peer.sh -k

# Get seed peer status and enode info
./run_seed_peer.sh -t

# Restart seed peer
./run_seed_peer.sh -r

# Show help
./run_seed_peer.sh -h
```

### Configuration

Edit the seed peer script to adjust settings:

```bash
# In run_seed_peer.sh
TARGET_SYNC_BLOCKS=1500  # Sync more blocks than your tests need
SEED_PEER_PORT=30304     # P2P port (avoid conflicts)
SEED_PEER_HTTP_PORT=8546 # HTTP RPC port
MAX_PEERS=25             # Limit peers for stability
```

## Benefits

### Speed Improvements
- **Peer Discovery**: Instant vs 2-5 minutes
- **Sync Start**: Immediate vs waiting for peers
- **Test Reliability**: Consistent performance across runs

### Typical Timeline Comparison

**Without Seed Peer:**
```
Peer Discovery: 3-5 minutes
Sync Start: 1-2 minutes  
First 100 blocks: 5-10 minutes
Total: 9-17 minutes
```

**With Seed Peer:**
```
Peer Discovery: 5-10 seconds
Sync Start: 10-20 seconds
First 100 blocks: 3-5 minutes  
Total: 4-6 minutes
```

## Troubleshooting

### Seed Peer Not Starting
```bash
# Check if port is available
lsof -i :30304
lsof -i :8546

# Check logs
tail -f seed_peer/logs/seed_peer.log
tail -f seed_peer/logs/seed_peer_startup.log
```

### Test Scripts Not Finding Seed Peer
```bash
# Verify seed peer is running
cd seed_peer
./run_seed_peer.sh -t

# Check enode file exists
ls -la seed_peer_enode.txt
cat seed_peer_enode.txt
```

### Seed Peer Sync Issues
```bash
# Restart with fresh data
./run_seed_peer.sh -k
rm -rf seed_peer_data
./run_seed_peer.sh -s

# Check peer connections
curl -s http://127.0.0.1:8546 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"net_peerCount","params":[],"id":1}'
```

## Advanced Usage

### Running Multiple Environments

For testing on different machines, you can export the seed peer:

```bash
# Get enode from seed peer machine
./run_seed_peer.sh -t

# On test machine, manually set enode
SEED_PEER_ENODE="enode://abc123...@192.168.1.100:30304"
```

### Persistent Seed Peer

To keep the seed peer running between test sessions:

```bash
# Start seed peer in background
nohup ./run_seed_peer.sh -s > seed_peer_output.log 2>&1 &

# Or use screen/tmux
screen -S seed_peer
./run_seed_peer.sh -s
# Ctrl+A, D to detach
```

### Monitoring Performance

```bash
# Monitor seed peer metrics
curl http://127.0.0.1:9046/debug/metrics/prometheus

# Check sync status
curl -s http://127.0.0.1:8546 -X POST -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"eth_syncing","params":[],"id":1}'
```

## Best Practices

1. **Start seed peer first**: Always ensure seed peer is running before tests
2. **Monitor resources**: Seed peer uses disk space and memory continuously  
3. **Regular cleanup**: Restart seed peer periodically to prevent state bloat
4. **Network stability**: Ensure seed peer has stable internet connection
5. **Port management**: Avoid port conflicts with other services

## Integration with CI/CD

For automated testing environments:

```bash
#!/bin/bash
# ci_test_setup.sh

# Start seed peer
cd seed_peer
./run_seed_peer.sh -s

# Wait for seed peer to be ready
sleep 30

# Run tests
cd ../native_bsc_node_startup
./bsc_performance_verification_with_seed_peer.sh

# Cleanup
cd ../seed_peer
./run_seed_peer.sh -k
```

## File Structure

```
perf_comparision/
├── seed_peer/
│   ├── run_seed_peer.sh           # Main seed peer script
│   ├── seed_peer_enode.txt        # Auto-generated enode info
│   ├── seed_peer_data/            # Blockchain data directory
│   └── logs/                      # Seed peer logs
├── native_bsc_node_startup/
│   ├── bsc_performance_verification_with_seed_peer.sh
│   └── ... (other files)
└── revm_bsc_node_startup/
    ├── revmffi_bsc_perf_verification_with_seed_peer.sh
    └── ... (other files)
```

## Performance Tuning

### Seed Peer Optimization
```bash
# Increase sync blocks for better peer data
TARGET_SYNC_BLOCKS=2000

# Adjust peer limits
MAX_PEERS=50

# Use fast sync mode for initial setup
# (Edit script to use --syncmode fast initially)
```

### Test Node Optimization
```bash
# Increase maxpeers for faster sync
--maxpeers 50

# Use additional static nodes
# (Scripts automatically include seed peer in static-nodes.json)
```

This seed peer setup provides a robust foundation for consistent, fast BSC node testing. The initial setup time is invested once, and all subsequent tests benefit from immediate peer connectivity. 