#![cfg(feature = "flashbots")]

use alloy::{
    consensus::constants::GWEI_TO_WEI,
    eips::{BlockId, Encodable2718},
    network::EthereumWallet,
    primitives::{B256, U256},
    providers::{
        ext::MevApi,
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, Provider, ProviderBuilder, SendableTx,
    },
    rpc::types::{
        mev::{EthCallBundle, EthSendBundle},
        TransactionRequest,
    },
    signers::Signer,
};
use init4_bin_base::{
    deps::tracing_subscriber::{
        fmt, layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter, Layer,
    },
    utils::signer::LocalOrAws,
};
use std::{
    env,
    sync::LazyLock,
    time::{Duration, Instant},
};
use url::Url;

/// Hoodi endpoints
static TITANBUILDER_HOODI_RPC: LazyLock<Url> = LazyLock::new(|| {
    Url::parse("https://rpc-hoodi.titanbuilder.xyz/").expect("valid flashbots url")
});

static HOODI_HOST_RPC: LazyLock<Url> = LazyLock::new(|| {
    Url::parse("https://ethereum-hoodi-rpc.publicnode.com").expect("valid hoodi url")
});

/// Pecorino endpoints
static PECORINO_RBUILDER: LazyLock<Url> = LazyLock::new(|| {
    Url::parse("https://host-builder-rpc.pecorino.signet.sh").expect("valid pecorino rbuilder url")
});

static PECORINO_HOST_RPC: LazyLock<Url> = LazyLock::new(|| {
    Url::parse("https://host-rpc.pecorino.signet.sh").expect("valid pecorino url")
});

type HoodiProvider = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    alloy::providers::RootProvider,
>;

#[allow(clippy::type_complexity)]
fn get_hoodi_host(builder_key: LocalOrAws) -> HoodiProvider {
    ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http(HOODI_HOST_RPC.clone())
}
    
#[tokio::test]
#[ignore = "integration test"]
async fn test_send_valid_bundle_hoodi() {
    setup_logging();

    let key_from_env = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&key_from_env, Some(560048))
        .await
        .expect("failed to load builder key");

    let flashbots = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http(TITANBUILDER_HOODI_RPC.clone());

    let hoodi = get_hoodi_host(builder_key.clone());

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(0u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((2 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    let SendableTx::Envelope(tx) = hoodi.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };

    let block = hoodi.get_block(BlockId::latest()).await.unwrap().unwrap();
    let target_block = block.number() + 1;

    let bundle = EthSendBundle {
        txs: vec![tx.encoded_2718().into()],
        block_number: target_block,
        ..Default::default()
    };

    let result = flashbots
        .send_bundle(bundle)
        .with_auth(builder_key.clone())
        .await;
    dbg!(result.as_ref().unwrap());
    assert!(result.is_ok(), "should send bundle: {:#?}", result);
    assert!(result.unwrap().is_some(), "should have bundle hash");
    // assert_tx_included(&hoodi, tx.tx_hash().clone(), 120).await;
}

//
// Pecorino rbuilder tests
//
#[tokio::test]
#[ignore = "integration test"]
async fn test_sim_bundle_pecorino() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(3151908))
        .await
        .expect("failed to load builder key");

    let flashbots = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http(PECORINO_RBUILDER.clone());

    let pecorino = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http(PECORINO_HOST_RPC.clone());

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(0u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((2 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    let SendableTx::Envelope(tx) = pecorino.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    dbg!("prepared transaction request", tx.clone());

    let block = pecorino
        .get_block(BlockId::latest())
        .await
        .unwrap()
        .unwrap();
    let target_block = block.number() + 1;
    dbg!("preparing bundle for", target_block);

    let bundle = EthCallBundle {
        txs: vec![tx.encoded_2718().into()],
        block_number: target_block,
        ..Default::default()
    };

    // FAIL: This test currently fails - why?
    // thread 'test_sim_bundle_pecorino' panicked at tests/flashbots.rs:610:17:
    // called `Result::unwrap()` on an `Err` value: ErrorResp(ErrorPayload { code: -32601, message: "Method not found", data: None })
    let result = flashbots
        .call_bundle(bundle)
        .with_auth(builder_key.clone())
        .await;
    dbg!(result.unwrap());
}

#[tokio::test]
#[ignore = "integration test"]
async fn test_send_bundle_pecorino() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(3151908))
        .await
        .expect("failed to load builder key");

    let flashbots = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http(PECORINO_RBUILDER.clone());

    let pecorino = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://rpc.pecorino.signet.sh".parse().unwrap());

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(0u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((2 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    let SendableTx::Envelope(tx) = pecorino.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    dbg!("prepared transaction request", tx.clone());

    let block = pecorino
        .get_block(BlockId::latest())
        .await
        .unwrap()
        .unwrap();
    let target_block = block.number() + 1;
    dbg!("preparing bundle for", target_block);

    let bundle = EthSendBundle {
        txs: vec![tx.encoded_2718().into()],
        block_number: target_block,
        ..Default::default()
    };

    let result = flashbots
        .send_bundle(bundle)
        .with_auth(builder_key.clone())
        .await;
    dbg!(result.as_ref().unwrap());
    assert!(result.is_ok(), "should send bundle: {:#?}", result);
    assert!(result.unwrap().is_some(), "should have bundle hash");
}

/// Asserts that a tx was included in Sepolia within `deadline` seconds.
async fn assert_tx_included(sepolia: &HoodiProvider, tx_hash: B256, deadline: u64) {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(deadline);
    let mut found = false;

    loop {
        let n = Instant::now();
        if n >= deadline {
            break;
        }

        match sepolia.get_transaction_by_hash(tx_hash).await {
            Ok(Some(_tx)) => {
                found = true;
                break;
            }
            Ok(None) => {
                // Not yet present; wait and retry
                dbg!("transaction not yet seen");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(err) => {
                // Transient error querying the provider; log and retry
                eprintln!("warning: error querying tx: {}", err);
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }

    assert!(
        found,
        "transaction was not seen by the provider within {:?} seconds",
        deadline
    );
}

/// Initializes logger for printing during testing
pub fn setup_logging() {
    let filter = EnvFilter::from_default_env();
    let fmt = fmt::layer().with_filter(filter);
    let registry = registry().with(fmt);
    let _ = registry.try_init();
}
