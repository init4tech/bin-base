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
        mev::{
            BundleItem, EthCallBundle, EthSendBundle, Inclusion, MevSendBundle, Privacy,
            ProtocolVersion,
        },
        TransactionRequest,
    },
    signers::{local::PrivateKeySigner, Signer},
};
use init4_bin_base::{
    deps::tracing::debug,
    deps::tracing_subscriber::{
        fmt, layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter, Layer,
    },
    utils::{flashbots::Flashbots, signer::LocalOrAws},
};
use std::{
    env,
    sync::LazyLock,
    time::{Duration, Instant},
};
use url::Url;

static FLASHBOTS_URL: LazyLock<Url> = LazyLock::new(|| {
    Url::parse("https://relay-sepolia.flashbots.net").expect("valid flashbots url")
});

static DEFAULT_BUILDER_KEY: LazyLock<LocalOrAws> = LazyLock::new(|| {
    LocalOrAws::Local(PrivateKeySigner::from_bytes(&B256::repeat_byte(0x02)).unwrap())
});

static TEST_PROVIDER: LazyLock<Flashbots> = LazyLock::new(get_default_test_provider);

fn get_default_test_provider() -> Flashbots {
    Flashbots::new(FLASHBOTS_URL.clone(), DEFAULT_BUILDER_KEY.clone())
}

type SepoliaProvider = FillProvider<
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
fn get_sepolia_host(builder_key: LocalOrAws) -> SepoliaProvider {
    ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http(
            "https://ethereum-sepolia-rpc.publicnode.com"
                .parse()
                .unwrap(),
        )
}

#[tokio::test]
#[ignore = "integration test"]
async fn test_simulate_valid_bundle_sepolia() {
    let flashbots = &*TEST_PROVIDER;
    let sepolia = get_sepolia_host(DEFAULT_BUILDER_KEY.clone());

    let req = TransactionRequest::default()
        .to(DEFAULT_BUILDER_KEY.address())
        .value(U256::from(1u64))
        .gas_limit(51_000)
        .from(DEFAULT_BUILDER_KEY.address());
    let SendableTx::Envelope(tx) = sepolia.fill(req).await.unwrap() else {
        panic!("expected filled tx");
    };
    let tx_bytes = tx.encoded_2718().into();

    let latest_block = sepolia
        .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
        .await
        .unwrap()
        .unwrap()
        .number();

    let bundle_body = vec![BundleItem::Tx {
        tx: tx_bytes,
        can_revert: false,
    }];
    let bundle = MevSendBundle::new(latest_block, Some(0), ProtocolVersion::V0_1, bundle_body);

    let err = flashbots
        .simulate_bundle(&bundle)
        .await
        .unwrap_err()
        .to_string();
    // If we have hit this point, we have succesfully authed to the flashbots
    // api via header
    assert!(
        err.contains("insufficient funds for gas"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
#[ignore = "integration test"]
async fn test_send_valid_bundle_sepolia() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(11155111))
        .await
        .expect("failed to load builder key");

    let flashbots = Flashbots::new(FLASHBOTS_URL.clone(), builder_key.clone());
    let sepolia = get_sepolia_host(builder_key.clone());

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(1u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((2 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    sepolia.estimate_gas(req.clone()).await.unwrap();

    let SendableTx::Envelope(tx) = sepolia.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    let tx_bytes = tx.encoded_2718().into();

    let latest_block = sepolia
        .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
        .await
        .unwrap()
        .unwrap()
        .number();
    // Give ourselves a buffer: target a couple blocks out to avoid timing edges
    let target_block = latest_block + 1;

    // Assemble the bundle and target it to the latest block
    let bundle_body = vec![BundleItem::Tx {
        tx: tx_bytes,
        can_revert: false,
    }];
    let mut bundle = MevSendBundle::new(
        target_block,
        Some(target_block + 5),
        ProtocolVersion::V0_1,
        bundle_body,
    );
    bundle.inclusion = Inclusion::at_block(target_block);

    // bundle.privacy = Some(Privacy::default().with_builders(Some(vec![
    //     "flashbots".to_string(),
    //     "rsync".to_string(),
    //     "Titan".to_string(),
    //     "beaverbuild.org".to_string(),
    // ])));

    dbg!(latest_block);
    dbg!(
        &bundle.inclusion.block_number(),
        &bundle.inclusion.max_block_number()
    );

    flashbots.simulate_bundle(&bundle).await.unwrap();

    let bundle_resp = flashbots.send_bundle(&bundle).await.unwrap();
    assert!(bundle_resp.bundle_hash != B256::ZERO);
    dbg!(bundle_resp);

    assert_tx_included(&sepolia, tx.hash().clone(), 15).await;
}

#[tokio::test]
#[ignore = "integration test"]
async fn test_send_valid_bundle_mainnet() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");

    let builder_key = LocalOrAws::load(&raw_key, None)
        .await
        .expect("failed to load builder key");
    debug!(builder_key_address = ?builder_key.address(), "loaded builder key");

    let flashbots = Flashbots::new(
        Url::parse("https://relay.flashbots.net").unwrap(),
        builder_key.clone(),
    );
    debug!(?flashbots.relay_url, "created flashbots provider");

    let mainnet = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://cloudflare-eth.com".parse().unwrap());

    // Build a valid transaction to bundle
    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(1u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((2 * GWEI_TO_WEI).into())
        .from(builder_key.address());
    dbg!(req.clone());

    // Estimate gas will fail if this wallet isn't properly funded for this TX.
    let gas_estimates = mainnet.estimate_gas(req.clone()).await.unwrap();
    dbg!(gas_estimates);

    let SendableTx::Envelope(tx) = mainnet.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    dbg!(req.clone());

    let tx_bytes = tx.encoded_2718().into();
    dbg!(tx.hash());

    // Fetch latest block info to build a valid target block for the bundle
    let latest_block = mainnet
        .get_block_by_number(alloy::eips::BlockNumberOrTag::Latest)
        .await
        .unwrap()
        .unwrap()
        .number();
    let target_block = latest_block + 1;

    // Assemble the bundle and target it to the latest block
    let bundle_body = vec![BundleItem::Tx {
        tx: tx_bytes,
        can_revert: false,
    }];
    let mut bundle = MevSendBundle::new(target_block, None, ProtocolVersion::V0_1, bundle_body);
    bundle.inclusion = Inclusion::at_block(target_block);
    bundle.privacy = Some(Privacy::default().with_builders(Some(vec!["flashbots".to_string()])));

    let resp = flashbots
        .send_bundle(&bundle)
        .await
        .expect("should send bundle");
    dbg!(&resp);

    assert!(resp.bundle_hash != B256::ZERO);
}

#[tokio::test]
#[ignore = "integration test"]
async fn test_alloy_flashbots_sepolia() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(11155111))
        .await
        .expect("failed to load builder key");

    let flashbots = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://relay-sepolia.flashbots.net".parse().unwrap());

    let sepolia_host = get_sepolia_host(builder_key.clone());

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(0u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((20 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    let block = sepolia_host
        .get_block(BlockId::latest())
        .await
        .unwrap()
        .unwrap();
    let target_block = block.number() + 1;
    dbg!("preparing bundle for", target_block);

    let SendableTx::Envelope(tx) = sepolia_host.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    dbg!("prepared transaction request", tx.clone());
    let tx_bytes = tx.encoded_2718();

    let bundle = EthSendBundle {
        txs: vec![tx_bytes.clone().into()],
        block_number: target_block,
        ..Default::default()
    };

    let call_bundle = EthCallBundle {
        txs: vec![tx_bytes.clone().into()],
        block_number: target_block,
        ..Default::default()
    };
    let sim = flashbots
        .call_bundle(call_bundle)
        .with_auth(builder_key.clone());
    dbg!(sim.await.unwrap());

    let result = flashbots.send_bundle(bundle).with_auth(builder_key.clone());
    dbg!(result.await.unwrap());
}

#[tokio::test]
#[ignore = "integration test"]
async fn test_mev_endpoints_sepolia() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(11155111))
        .await
        .expect("failed to load builder key");

    let flashbots = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://relay-sepolia.flashbots.net".parse().unwrap());

    // TEMP: Keeping this around because alloy flashbots doesn't have a simulate endpoint for `mev_simBundle`.
    let old_flashbots = Flashbots::new(
        "https://relay-sepolia.flashbots.net".parse().unwrap(),
        builder_key.clone(),
    );

    let sepolia_host = get_sepolia_host(builder_key.clone());

    let block = sepolia_host
        .get_block(BlockId::latest())
        .await
        .unwrap()
        .unwrap();
    let target_block = block.number() + 1;
    dbg!("preparing bundle for", target_block);

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(0u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((20 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    let SendableTx::Envelope(tx) = sepolia_host.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    dbg!("prepared transaction request", tx.clone());
    let tx_bytes = tx.encoded_2718();

    let bundle = MevSendBundle::new(
        target_block,
        None,
        ProtocolVersion::V0_1,
        vec![BundleItem::Tx {
            tx: tx_bytes.clone().into(),
            can_revert: false,
        }],
    );
    dbg!("bundle contents", &bundle);

    let _ = old_flashbots.simulate_bundle(&bundle).await.unwrap();

    let result = flashbots
        .send_mev_bundle(bundle)
        .with_auth(builder_key.clone());
    dbg!("send mev bundle:", result.await.unwrap());
}

#[tokio::test]
#[ignore = "integration test"]
async fn test_alloy_flashbots_mainnet() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(11155111))
        .await
        .expect("failed to load builder key");

    let flashbots = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://relay.flashbots.net".parse().unwrap());

    let mainnet = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://ethereum-rpc.publicnode.com".parse().unwrap());

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(0u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((2 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    let block = mainnet.get_block(BlockId::latest()).await.unwrap().unwrap();
    let target_block = block.number() + 1;
    let SendableTx::Envelope(tx) = mainnet.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    dbg!("prepared transaction request", tx.clone(), target_block);
    let tx_bytes = tx.encoded_2718();

    let bundle = EthSendBundle {
        txs: vec![tx_bytes.clone().into()],
        block_number: target_block,
        ..Default::default()
    };

    let call_bundle = EthCallBundle {
        txs: vec![tx_bytes.clone().into()],
        block_number: target_block,
        ..Default::default()
    };

    let sim = flashbots
        .call_bundle(call_bundle)
        .with_auth(builder_key.clone());
    dbg!(sim.await.unwrap());

    let result = flashbots.send_bundle(bundle).with_auth(builder_key.clone());
    dbg!(result.await.unwrap());
}

#[tokio::test]
#[ignore = "integration test"]
pub async fn test_send_single_tx_sepolia() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(11155111))
        .await
        .expect("failed to load builder key");

    let sepolia_host = get_sepolia_host(builder_key.clone());

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(0u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((2 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    let SendableTx::Envelope(tx) = sepolia_host.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    dbg!("prepared transaction request", tx.clone());
    let tx_bytes = tx.encoded_2718();

    let pending_tx = sepolia_host
        .send_raw_transaction(&tx_bytes)
        .await
        .expect("should send tx")
        .watch()
        .await
        .unwrap();
    dbg!(pending_tx);
}

#[tokio::test]
#[ignore = "integration test"]
pub async fn test_send_single_tx_hoodi() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(560048))
        .await
        .expect("failed to load builder key");

    let req = TransactionRequest::default()
        .to(builder_key.address())
        .value(U256::from(0u64))
        .gas_limit(21_000)
        .max_fee_per_gas((50 * GWEI_TO_WEI).into())
        .max_priority_fee_per_gas((2 * GWEI_TO_WEI).into())
        .from(builder_key.address());

    let hoodi = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://ethereum-hoodi-rpc.publicnode.com".parse().unwrap());

    let SendableTx::Envelope(tx) = hoodi.fill(req.clone()).await.unwrap() else {
        panic!("expected filled tx");
    };
    dbg!("prepared transaction request", tx.clone());
    let tx_bytes = tx.encoded_2718();

    let pending_tx = hoodi
        .send_raw_transaction(&tx_bytes)
        .await
        .expect("should send tx")
        .watch()
        .await
        .unwrap();
    dbg!(pending_tx);
}

#[tokio::test]
#[ignore = "integration test"]
async fn test_send_valid_bundle_hoodi() {
    setup_logging();

    let raw_key = env::var("BUILDER_KEY").expect("BUILDER_KEY must be set");
    let builder_key = LocalOrAws::load(&raw_key, Some(560048))
        .await
        .expect("failed to load builder key");

    let flashbots = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://boost-relay-hoodi.flashbots.net".parse().unwrap());

    let hoodi = ProviderBuilder::new()
        .wallet(builder_key.clone())
        .connect_http("https://ethereum-hoodi-rpc.publicnode.com".parse().unwrap());

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
    dbg!("prepared transaction request", tx.clone());
    let tx_bytes = tx.encoded_2718();

    let block = hoodi.get_block(BlockId::latest()).await.unwrap().unwrap();
    let target_block = block.number() + 1;
    dbg!("preparing bundle for", target_block);

    // let call_bundle = EthCallBundle {
    //     txs: vec![tx_bytes.clone().into()],
    //     block_number: target_block,
    //     ..Default::default()
    // };

    // let sim = flashbots
    //     .call_bundle(call_bundle)
    //     .with_auth(builder_key.clone());
    // dbg!(sim.await.unwrap());

    let bundle = EthSendBundle {
        txs: vec![tx_bytes.clone().into()],
        block_number: target_block,
        ..Default::default()
    };
    
    let result = flashbots.send_bundle(bundle).with_auth(builder_key.clone());
    dbg!(result.await.unwrap());
}

/// Asserts that a tx was included in Sepolia within `deadline` seconds.
async fn assert_tx_included(sepolia: &SepoliaProvider, tx_hash: B256, deadline: u64) {
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
    // Initialize logging
    let filter = EnvFilter::from_default_env();
    let fmt = fmt::layer().with_filter(filter);
    let registry = registry().with(fmt);
    let _ = registry.try_init();
}
