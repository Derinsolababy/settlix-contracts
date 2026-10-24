# Settlix smart contracts
#
#   make            build every contract to optimized wasm
#   make test       run the full test suite
#   make check      fmt + clippy + test, exactly what CI runs
#   make deploy     deploy and wire all contracts (NETWORK=testnet by default)
#   make verify     check deployed contracts match this source (NETWORK=testnet)

NETWORK ?= testnet
SOURCE  ?= settlix-deployer
FEE_BPS ?= 50
WASM_DIR = $(or $(CARGO_TARGET_DIR),target)/wasm32v1-none/release

.PHONY: all build test fmt fmt-check lint check clean deploy verify sizes help

all: build

help:
	@grep -E '^#   ' Makefile | sed 's/^#   //'

build:
	stellar contract build

test:
	cargo test --workspace

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets -- -D warnings

check: fmt-check lint test

sizes: build
	@ls -l $(WASM_DIR)/*.wasm | awk '{n=split($$9,p,"/"); printf "%-32s %6d bytes\n", p[n], $$5}'

deploy:
	NETWORK=$(NETWORK) SOURCE=$(SOURCE) FEE_BPS=$(FEE_BPS) ./scripts/deploy.sh

verify:
	NETWORK=$(NETWORK) ./scripts/verify.sh

clean:
	cargo clean
