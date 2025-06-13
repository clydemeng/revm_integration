module benchmark_go_call_ffi

go 1.23.0

replace (
	github.com/cometbft/cometbft => github.com/bnb-chain/greenfield-cometbft v1.3.1
	github.com/ethereum/c-kzg-4844 => github.com/ethereum/c-kzg-4844 v0.4.0
	github.com/ethereum/go-ethereum => github.com/bnb-chain/bsc v1.5.10
	github.com/gogo/protobuf => github.com/gogo/protobuf v1.3.2
	github.com/mitchellh/osext => github.com/kardianos/osext v0.0.0-20190222173326-2bc1f35cddc0
	github.com/prysmaticlabs/fastssz => github.com/prysmaticlabs/fastssz v0.0.0-20221107182844-78142813af44 // indirect
	github.com/prysmaticlabs/prysm/v5 => github.com/prysmaticlabs/prysm/v5 v5.0.3 // indirect
	github.com/syndtr/goleveldb v1.0.1 => github.com/syndtr/goleveldb v1.0.1-0.20210819022825-2ae1ddf74ef7
	//github.com/grpc-ecosystem/grpc-gateway/v2 => github.com/prysmaticlabs/grpc-gateway/v2 v2.3.1-0.20210702154020-550e1cd83ec1
	github.com/tendermint/tendermint => github.com/bnb-chain/tendermint v0.31.16
	github.com/wercker/journalhook => github.com/wercker/journalhook v0.0.0-20230927020745-64542ffa4117
)

require github.com/ethereum/go-ethereum v1.13.5

require (
	github.com/holiman/uint256 v1.3.2 // indirect
	golang.org/x/crypto v0.36.0 // indirect
	golang.org/x/sys v0.31.0 // indirect
)
