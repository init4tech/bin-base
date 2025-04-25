#![deny(proc_macro_derive_resolution_fallback)]

use init4_bin_base::utils::from_env::FromEnv;

#[derive(Debug, FromEnv)]
pub struct MyCfg {
    #[from_env(var = "COOL_DUDE", desc = "Some u8 we like :o)")]
    pub my_cool_u8: u8,

    #[from_env(var = "CHUCK", desc = "Charles is a u64")]
    pub charles: u64,

    #[from_env(var = "PERFECT", desc = "A bold and neat string", infallible)]
    pub strings_cannot_fail: String,

    #[from_env(
        var = "MAYBE_NOT_NEEDED",
        desc = "This is an optional string",
        optional,
        infallible
    )]
    pub maybe_not_needed: Option<String>,
}

#[test]
fn basic_inventory() {
    let inv = MyCfg::inventory();
    assert_eq!(inv.len(), 4);
}
