//! Integration tests for stellar-router
//!
//! These tests run against Stellar testnet and verify end-to-end functionality.
//!
//! ## Running the tests
//!
//! ### Prerequisites
//! 1. Install stellar-cli: `cargo install --locked stellar-cli`
//! 2. Build WASM contracts: `cargo build --target wasm32-unknown-unknown --release`
//!
//! ### Run all integration tests
//! ```bash
//! cargo test --test integration_tests -- --ignored --test-threads=1
//! ```
//!
//! ### Run specific test
//! ```bash
//! cargo test --test integration_tests test_full_router_core_flow -- --ignored
//! ```
//!
//! ### Run with output
//! ```bash
//! cargo test --test integration_tests -- --ignored --nocapture --test-threads=1
//! ```
//!
//! ## Test Organization
//!
//! - `testnet_setup.rs` - Utilities for deploying and managing testnet resources
//! - `full_flow_test.rs` - Happy path end-to-end tests
//! - `failure_scenarios.rs` - Error handling and edge case tests
//!
//! ## Notes
//!
//! - Tests are marked with `#[ignore]` to prevent running in normal CI
//! - Use `--test-threads=1` to avoid testnet rate limits
//! - Each test creates fresh accounts via Friendbot
//! - Tests clean up after themselves but may leave contracts on testnet

mod integration {
    pub mod failure_scenarios;
    pub mod full_flow_test;
    pub mod testnet_setup;
}

#[cfg(test)]
mod quick_tests {
    use super::integration::testnet_setup::TestAccount;

    #[test]
    #[ignore]
    fn test_stellar_cli_available() {
        let output = std::process::Command::new("stellar")
            .arg("--version")
            .output()
            .expect("Failed to run stellar CLI - is it installed?");

        assert!(output.status.success(), "stellar CLI not working properly");
        let version = String::from_utf8_lossy(&output.stdout);
        println!("stellar CLI version: {}", version);
        assert!(version.contains("stellar"), "Unexpected stellar CLI output");
    }

    #[test]
    #[ignore]
    fn test_wasm_contracts_built() {
        let contracts = vec![
            "target/wasm32-unknown-unknown/release/router_core.wasm",
            "target/wasm32-unknown-unknown/release/router_registry.wasm",
            "target/wasm32-unknown-unknown/release/router_access.wasm",
            "target/wasm32-unknown-unknown/release/router_middleware.wasm",
            "target/wasm32-unknown-unknown/release/router_timelock.wasm",
            "target/wasm32-unknown-unknown/release/router_multicall.wasm",
        ];

        for contract in contracts {
            assert!(
                std::path::Path::new(contract).exists(),
                "Contract not found: {}. Run: cargo build --target wasm32-unknown-unknown --release",
                contract
            );
        }
        println!("✓ All WASM contracts found");
    }

    #[test]
    #[ignore]
    fn test_account_generation_and_funding() {
        println!("\n=== Testing Account Setup ===\n");

        let account = TestAccount::generate().expect("Failed to generate test account");
        println!("✓ Generated account: {}", account.address);

        account
            .fund()
            .expect("Failed to fund account via Friendbot");
        println!("✓ Account funded successfully");

        println!("\n=== Account Setup Test PASSED ===\n");
    }
}
