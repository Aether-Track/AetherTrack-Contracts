SHELL := bash
TARGETS := wasm32-unknown-unknown

.PHONY: build test clean fmt

build:
	cargo build --release --target $(TARGETS)

test:
	cargo test --features testutils

fmt:
	cargo fmt --all

clean:
	cargo clean

# Build and optimize WASM artifacts
wasm: build
	@mkdir -p artifacts
	cp target/wasm32-unknown-unknown/release/shipment_registry.wasm artifacts/
	cp target/wasm32-unknown-unknown/release/payment_escrow.wasm artifacts/
	@echo "WASM artifacts copied to ./artifacts/"

# Deploy to Stellar Testnet (requires soroban-cli)
deploy-testnet:
	@echo "Deploying ShipmentRegistry to Testnet..."
	soroban contract deploy \
		--wasm artifacts/shipment_registry.wasm \
		--source-account $$STELLAR_SECRET \
		--network testnet
