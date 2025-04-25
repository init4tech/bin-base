use from_env_derive::FromEnv;

#[derive(FromEnv, Debug)]
pub struct FromEnvTest {
    /// This is a guy named tony
    /// He is cool
    /// He is a good guy
    #[from_env(var = "FIELD1", desc = "Tony is cool and a u8")]
    pub tony: u8,

    /// This guy is named charles
    /// whatever.
    #[from_env(var = "FIELD2", desc = "Charles is a u64")]
    pub charles: u64,

    /// This is a guy named patrick
    #[from_env(var = "FIELD3", infallible, desc = "Patrick is a String")]
    pub patrick: String,

    /// This is a guy named oliver
    #[from_env(
        var = "FIELD4",
        optional,
        infallible,
        desc = "Oliver is an Option<String>"
    )]
    pub oliver: Option<String>,
}

#[derive(Debug, FromEnv)]
pub struct Nested {
    #[from_env(var = "FFFFFF", desc = "This is a guy named ffffff")]
    pub ffffff: String,

    /// Hi
    #[from_env(desc = "This is a guy named from_env_test")]
    pub from_env_test: FromEnvTest,
}

#[cfg(test)]
mod test {
    use super::*;
    use init4_bin_base::utils::from_env::{EnvItemInfo, FromEnv};

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

    fn assert_contains(vec: &Vec<&'static EnvItemInfo>, item: &EnvItemInfo) {
        let item = vec.iter().find(|i| i.var == item.var).unwrap();
        assert_eq!(item.var, item.var);
        assert_eq!(item.description, item.description);
        assert_eq!(item.optional, item.optional);
    }

    #[test]
    fn nested_inventory() {
        let fet_inv = FromEnvTest::inventory();
        assert_eq!(fet_inv.len(), 4);
        assert_contains(
            &fet_inv,
            &EnvItemInfo {
                var: "FIELD1",
                description: "Tony is cool and a u8",
                optional: false,
            },
        );
        assert_contains(
            &fet_inv,
            &EnvItemInfo {
                var: "FIELD2",
                description: "Charles is a u64",
                optional: false,
            },
        );
        assert_contains(
            &fet_inv,
            &EnvItemInfo {
                var: "FIELD3",
                description: "Patrick is a String",
                optional: false,
            },
        );
        assert_contains(
            &fet_inv,
            &EnvItemInfo {
                var: "FIELD4",
                description: "Oliver is an Option<String>",
                optional: true,
            },
        );

        let nest_inv = Nested::inventory();
        assert_eq!(nest_inv.len(), fet_inv.len() + 1);
        for item in fet_inv {
            assert_contains(&nest_inv, item);
        }
        assert_contains(
            &nest_inv,
            &EnvItemInfo {
                var: "FFFFFF",
                description: "This is a guy named ffffff",
                optional: false,
            },
        );
    }
}
