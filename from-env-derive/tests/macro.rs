use from_env_macro::FromEnv;

#[derive(FromEnv, Debug)]
pub struct FromEnvTest {
    /// This is a guy named tony
    /// He is cool
    /// He is a good guy
    #[from_env(var = "FIELD1", desc = "Tony is cool and a u8")]
    pub tony: u8,

    /// This guy is named charles
    /// whatever.
    #[from_env(var = "FIELD2")]
    pub charles: u64,

    /// This is a guy named patrick
    #[from_env(var = "FIELD3", infallible)]
    pub patrick: String,

    /// This is a guy named oliver
    #[from_env(var = "FIELD4", optional, infallible)]
    pub oliver: Option<String>,
}

#[derive(Debug, FromEnv)]
pub struct Nested {
    #[from_env(var = "FFFFFF")]
    pub ffffff: String,

    /// Hi
    pub from_env_test: FromEnvTest,
}

#[cfg(test)]
mod test {
    use super::*;
    use init4_bin_base::utils::from_env::FromEnv;

    #[test]
    fn load_nested() {
        unsafe {
            std::env::set_var("FIELD1", "1");
            std::env::set_var("FIELD2", "2");
            std::env::set_var("FIELD3", "3");
            std::env::set_var("FIELD4", "4");
            std::env::set_var("FFFFFF", "5");
        }

        let nested = Nested::from_env().unwrap();
        assert_eq!(nested.from_env_test.tony, 1);
        assert_eq!(nested.from_env_test.charles, 2);
        assert_eq!(nested.from_env_test.patrick, "3");
        assert_eq!(nested.from_env_test.oliver, Some("4".to_string()));
        assert_eq!(nested.ffffff, "5");

        unsafe {
            std::env::remove_var("FIELD4");
        }

        let nested = Nested::from_env().unwrap();
        assert_eq!(nested.from_env_test.tony, 1);
        assert_eq!(nested.from_env_test.charles, 2);
        assert_eq!(nested.from_env_test.patrick, "3");
        assert_eq!(nested.from_env_test.oliver, None);
        assert_eq!(nested.ffffff, "5");
    }
}
