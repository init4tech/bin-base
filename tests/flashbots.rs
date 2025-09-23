#![cfg(feature = "flashbots")]

use alloy::{
    eips::Encodable2718,
    network::EthereumWallet,
    primitives::{B256, U256},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller,
            WalletFiller,
        },
        Identity, Provider, ProviderBuilder, SendableTx,
    },
    rpc::types::{
        mev::{BundleItem, MevSendBundle, ProtocolVersion},
        TransactionRequest,
    },
    signers::{local::PrivateKeySigner, Signer},
};
use init4_bin_base::utils::{flashbots::Flashbots, signer::LocalOrAws};
use std::sync::LazyLock;
use url::Url;

static FLASHBOTS_URL: LazyLock<Url> = LazyLock::new(|| {
    Url::parse("https://relay-sepolia.flashbots.net:443").expect("valid flashbots url")
});
static BUILDER_KEY: LazyLock<LocalOrAws> = LazyLock::new(|| {
    LocalOrAws::Local(PrivateKeySigner::from_bytes(&B256::repeat_byte(0x02)).unwrap())
});
static TEST_PROVIDER: LazyLock<Flashbots> = LazyLock::new(get_test_provider);

fn get_test_provider() -> Flashbots {
    Flashbots::new(FLASHBOTS_URL.clone(), BUILDER_KEY.clone())
}

#[allow(clippy::type_complexity)]
fn get_sepolia() -> FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
        >,
        WalletFiller<EthereumWallet>,
    >,
    alloy::providers::RootProvider,
> {
    ProviderBuilder::new()
        .wallet(BUILDER_KEY.clone())
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
    let sepolia = get_sepolia();

    let req = TransactionRequest::default()
        .to(BUILDER_KEY.address())
        .value(U256::from(1u64))
        .gas_limit(51_000)
        .from(BUILDER_KEY.address());
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
        can_revert: true,
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
