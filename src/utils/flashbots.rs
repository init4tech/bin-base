//! A generic Flashbots bundle API wrapper.
use crate::utils::signer::LocalOrAws;
use alloy::{
    primitives::keccak256,
    rpc::{
        json_rpc::{Id, Response, ResponsePayload, RpcRecv, RpcSend},
        types::mev::{EthBundleHash, MevSendBundle, SimBundleResponse},
    },
    signers::Signer,
};
use init4_from_env_derive::FromEnv;
use reqwest::header::CONTENT_TYPE;
use std::borrow::Cow;

/// Configuration for the Flashbots provider.
#[derive(Debug, Clone, FromEnv)]
#[from_env(crate)]
pub struct FlashbotsConfig {
    /// Flashbots endpoint for privately submitting rollup blocks.
    #[from_env(
        var = "FLASHBOTS_ENDPOINT",
        desc = "Flashbots endpoint for privately submitting rollup blocks",
        optional
    )]
    pub flashbots_endpoint: Option<url::Url>,
}

impl FlashbotsConfig {
    /// Make a [`Flashbots`] instance from this config, using the specified signer.
    pub fn build(&self, signer: LocalOrAws) -> Option<Flashbots> {
        self.flashbots_endpoint
            .as_ref()
            .map(|url| Flashbots::new(url.clone(), signer))
    }
}

/// A basic provider for common Flashbots Relay endpoints.
#[derive(Debug)]
pub struct Flashbots {
    /// The base URL for the Flashbots API.
    pub relay_url: url::Url,

    /// Signer is loaded once at startup.
    signer: LocalOrAws,

    /// The reqwest client to use for requests.
    client: reqwest::Client,
}

impl Flashbots {
    /// Instantiate a new provider from the URL and signer.
    pub fn new(relay_url: url::Url, signer: LocalOrAws) -> Self {
        Self {
            relay_url,
            client: Default::default(),
            signer,
        }
    }

    /// Instantiate a new provider from the URL and signer, with a specific
    /// Reqwest client.
    pub const fn new_with_client(
        relay_url: url::Url,
        signer: LocalOrAws,
        client: reqwest::Client,
    ) -> Self {
        Self {
            relay_url,
            client,
            signer,
        }
    }

    /// Sends a bundle  via `mev_sendBundle`.
    pub async fn send_bundle(&self, bundle: &MevSendBundle) -> eyre::Result<EthBundleHash> {
        let resp = self.raw_call("mev_sendBundle", &[bundle]).await?;
        dbg!("sim bundle response", &resp);
        Ok(resp)
    }

    /// Simulate a bundle via `mev_simBundle`.
    pub async fn simulate_bundle(&self, bundle: &MevSendBundle) -> eyre::Result<()> {
        let resp: SimBundleResponse = self.raw_call("mev_simBundle", &[bundle]).await?;
        dbg!("send bundle response ###", resp);
        Ok(())
    }

    /// Make a raw JSON-RPC call with the Flashbots signature header to the
    /// method with the given params.
    async fn raw_call<Params: RpcSend, Payload: RpcRecv>(
        &self,
        method: &str,
        params: &Params,
    ) -> eyre::Result<Payload> {
        let req = alloy::rpc::json_rpc::Request::new(
            Cow::Owned(method.to_string()),
            Id::Number(1),
            params,
        );
        let body_bz = serde_json::to_vec(&req)?;
        drop(req);

        let value = self.compute_signature(&body_bz).await?;

        let resp = self
            .client
            .post(self.relay_url.as_str())
            .header(CONTENT_TYPE, "application/json")
            .header("X-Flashbots-Signature", value)
            .body(body_bz)
            .send()
            .await?;

        let resp: Response<Payload> = resp.json().await?;

        match resp.payload {
            ResponsePayload::Success(payload) => Ok(payload),
            ResponsePayload::Failure(err) => {
                eyre::bail!("flashbots error: {err}");
            }
        }
    }

    /// Builds an EIP-191 signature for the given body bytes. This signature is
    /// used to authenticate to the relay API via a header
    async fn compute_signature(&self, body_bz: &[u8]) -> Result<String, eyre::Error> {
        let payload = keccak256(body_bz).to_string();
        let signature = self.signer.sign_message(payload.as_ref()).await?;
        let address = self.signer.address();
        let value = format!("{address}:{signature}");
        Ok(value)
    }
}
