use init4_bin_base::{perms::OAuthConfig, utils::from_env::FromEnv};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cfg = OAuthConfig::from_env()?;
    let authenticator = cfg.authenticator();
    let token = authenticator.token();

    let _jh = authenticator.spawn();

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    dbg!(token.read());

    Ok(())
}
