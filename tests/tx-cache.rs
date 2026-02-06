#![cfg(feature = "perms")]

use init4_bin_base::perms::{tx_cache::{BuilderTxCache, BuilderTxCacheError}, SharedToken};
use signet_tx_cache::TxCacheError;

const URL: &str = "https://transactions.parmigiana.signet.sh";

#[ignore = "integration"]
#[tokio::test]
async fn test_tx_cache_get_bundles() {
    let client = BuilderTxCache::new_from_string(URL, SharedToken::empty()).unwrap();

    let bundles = client.get_bundles(None).await.unwrap_err();

    assert!(matches!(bundles, BuilderTxCacheError::TxCache(TxCacheError::NotOurSlot)));
}
