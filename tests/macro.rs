use from_env_macro::FromEnv;

#[derive(FromEnv, Debug)]
pub struct FromEnvTest {
    pub tony: String,

    #[from_env_var("FIELD2")]
    pub charles: u64,
}
