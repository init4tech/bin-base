use init4_bin_base::{
    perms::tx_cache::BuilderTxCache, perms::OAuthConfig, utils::from_env::FromEnv,
};
use signet_tx_cache::client::TxCache;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cfg = OAuthConfig::from_env()?;
    let authenticator = cfg.authenticator();
    let token = authenticator.token();

    let _jh = authenticator.spawn();

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let tx_cache = BuilderTxCache::new(TxCache::pecorino(), token);

    let bundles = tx_cache.get_bundles().await?;

    println!("Bundles: {bundles:#?}");

    Ok(())
}
