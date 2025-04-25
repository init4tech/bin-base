use std::{convert::Infallible, env::VarError, num::ParseIntError, str::FromStr};

/// The `derive(FromEnv)` macro.
///
/// This macro generates a [`FromEnv`] implementation for the struct it is
/// applied to. It will generate a `from_env` function that loads the struct
/// from the environment. It will also generate an `inventory` function that
/// returns a list of all environment variables that are required to load the
/// struct.
///
/// The macro also generates a `__EnvError` type that captures errors that can
/// occur when trying to create an instance of the struct from environment
/// variables. This error type is used in the `FromEnv` trait implementation.
///
/// ## Basics
///
/// There are a few usage requirements:
///
/// - Struct props MUST implement either [`FromEnvVar`] or [`FromEnv`].
/// - If the prop implements [`FromEnvVar`], it must be tagged as follows:
///     - `var = "ENV_VAR_NAME"`: The environment variable name to load.
///     - `desc = "description"`: A description of the environment variable.
/// - If the prop is an [`Option<T>`], it must be tagged as follows:
///     - `optional`
/// - If the prop's associated error type is [`Infallible`], it must be tagged
///   as follows:
///     - `infallible`
/// - If used within this crate (`init4_bin_base`), the entire struct must be
///   tagged with `#[from_env(crate)]` (see the [`SlotCalculator`] for an
///   example).
///
/// # Examples
///
/// The following example shows how to use the macro:
///
/// ```
/// # // I am unsure why we need this, as identical code works in
/// # // integration tests. However, compile test fails without it.
/// # #![allow(proc_macro_derive_resolution_fallback)]
/// use init4_bin_base::utils::from_env::{FromEnv};
///
/// #[derive(Debug, FromEnv)]
/// pub struct MyCfg {
///     #[from_env(var = "COOL_DUDE", desc = "Some u8 we like :o)")]
///     pub my_cool_u8: u8,
///
///     #[from_env(var = "CHUCK", desc = "Charles is a u64")]
///     pub charles: u64,
///
///     #[from_env(
///         var = "PERFECT",
///         desc = "A bold and neat string",
///         infallible,
///     )]
///     pub strings_cannot_fail: String,
///
///     #[from_env(
///         var = "MAYBE_NOT_NEEDED",
///         desc = "This is an optional string",
///         optional,
///         infallible,
///     )]
///     maybe_not_needed: Option<String>,
/// }
/// ```
///
/// This will generate a `FromEnv` implementation for the struct, and a
/// `MyCfgEnvError` type that is used to represent errors that can occur when
/// loading from the environment. The error generated will look like this:
///
/// ```ignore
/// pub enum MyCfgEnvError {
///     MyCoolU8(<u8 as FromEnvVar>::Error),
///     Charles(<u64 as FromEnvVar>::Error),
///     // No variants for infallible errors.
/// }
/// ```
///
/// [`Infallible`]: std::convert::Infallible
/// [`SlotCalculator`]: crate::utils::SlotCalculator
pub use init4_from_env_derive::FromEnv;

/// Details about an environment variable. This is used to generate
/// documentation for the environment variables and by the [`FromEnv`] trait to
/// check if necessary environment variables are present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvItemInfo {
    /// The environment variable name.
    pub var: &'static str,
    /// A description of the environment variable function in the CFG.
    pub description: &'static str,
    /// Whether the environment variable is optional or not.
    pub optional: bool,
}

/// Error type for loading from the environment. See the [`FromEnv`] trait for
/// more information.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FromEnvErr<Inner> {
    /// The environment variable is missing.
    #[error("Error reading variable {0}: {1}")]
    EnvError(String, VarError),
    /// The environment variable is empty.
    #[error("Environment variable {0} is empty")]
    Empty(String),
    /// The environment variable is present, but the value could not be parsed.
    #[error("Failed to parse environment variable {0}")]
    ParseError(#[from] Inner),
}

impl FromEnvErr<Infallible> {
    /// Convert the error into another error type.
    pub fn infallible_into<T>(self) -> FromEnvErr<T> {
        match self {
            Self::EnvError(s, e) => FromEnvErr::EnvError(s, e),
            Self::Empty(s) => FromEnvErr::Empty(s),
            Self::ParseError(_) => unreachable!(),
        }
    }
}

impl<Inner> FromEnvErr<Inner> {
    /// Create a new error from another error type.
    pub fn from<Other>(other: FromEnvErr<Other>) -> Self
    where
        Inner: From<Other>,
    {
        match other {
            FromEnvErr::EnvError(s, e) => Self::EnvError(s, e),
            FromEnvErr::Empty(s) => Self::Empty(s),
            FromEnvErr::ParseError(e) => Self::ParseError(Inner::from(e)),
        }
    }

    /// Map the error to another type. This is useful for converting the error
    /// type to a different type, while keeping the other error information
    /// intact.
    pub fn map<New>(self, f: impl FnOnce(Inner) -> New) -> FromEnvErr<New> {
        match self {
            Self::EnvError(s, e) => FromEnvErr::EnvError(s, e),
            Self::Empty(s) => FromEnvErr::Empty(s),
            Self::ParseError(e) => FromEnvErr::ParseError(f(e)),
        }
    }

    /// Missing env var.
    pub fn env_err(var: &str, e: VarError) -> Self {
        Self::EnvError(var.to_string(), e)
    }

    /// Empty env var.
    pub fn empty(var: &str) -> Self {
        Self::Empty(var.to_string())
    }

    /// Error while parsing.
    pub const fn parse_error(err: Inner) -> Self {
        Self::ParseError(err)
    }
}

/// Convenience function for parsing a value from the environment, if present
/// and non-empty.
pub fn parse_env_if_present<T: FromStr>(env_var: &str) -> Result<T, FromEnvErr<T::Err>> {
    let s = std::env::var(env_var).map_err(|e| FromEnvErr::env_err(env_var, e))?;

    if s.is_empty() {
        Err(FromEnvErr::empty(env_var))
    } else {
        s.parse().map_err(Into::into)
    }
}

/// Trait for loading from the environment.
///
/// This trait is for structs or other complex objects, that need to be loaded
/// from the environment. It expects that
///
/// - The struct is [`Sized`] and `'static`.
/// - The struct elements can be parsed from strings.
/// - Struct elements are at fixed env vars, known by the type at compile time.
///
/// As such, unless the env is modified, these are essentially static runtime
/// values.
pub trait FromEnv: core::fmt::Debug + Sized + 'static {
    /// Error type produced when loading from the environment.
    type Error: core::error::Error + Clone;

    /// Get the required environment variable names for this type.
    ///
    /// ## Note
    ///
    /// This MUST include the environment variable names for all fields in the
    /// struct, including optional vars.
    fn inventory() -> Vec<&'static EnvItemInfo>;

    /// Get a list of missing environment variables.
    ///
    /// This will check all environment variables in the inventory, and return
    /// a list of those that are non-optional and missing. This is useful for
    /// reporting missing environment variables.
    fn check_inventory() -> Result<(), Vec<&'static EnvItemInfo>> {
        let mut missing = Vec::new();
        for var in Self::inventory() {
            if std::env::var(var.var).is_err() && !var.optional {
                missing.push(var);
            }
        }
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }

    /// Load from the environment.
    fn from_env() -> Result<Self, FromEnvErr<Self::Error>>;
}

/// Trait for loading primitives from the environment. These are simple types
/// that should correspond to a single environment variable. It has been
/// implemented for common integer types, [`String`], [`url::Url`],
/// [`tracing::Level`], and [`std::time::Duration`].
///
/// It aims to make [`FromEnv`] implementations easier to write, by providing a
/// default implementation for common types.
pub trait FromEnvVar: core::fmt::Debug + Sized + 'static {
    /// Error type produced when parsing the primitive.
    type Error: core::error::Error;

    /// Load the primitive from the environment at the given variable.
    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>>;
}

impl<T> FromEnvVar for Option<T>
where
    T: FromEnvVar,
{
    type Error = T::Error;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        match std::env::var(env_var) {
            Ok(s) if s.is_empty() => Ok(None),
            Ok(_) => T::from_env_var(env_var).map(Some),
            Err(_) => Ok(None),
        }
    }
}

impl FromEnvVar for String {
    type Error = std::convert::Infallible;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        std::env::var(env_var).map_err(|_| FromEnvErr::empty(env_var))
    }
}

impl FromEnvVar for std::time::Duration {
    type Error = ParseIntError;

    fn from_env_var(s: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        u64::from_env_var(s).map(Self::from_millis)
    }
}

impl<T> FromEnvVar for Vec<T>
where
    T: FromEnvVar,
{
    type Error = T::Error;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        let s = std::env::var(env_var).map_err(|e| FromEnvErr::env_err(env_var, e))?;
        if s.is_empty() {
            return Ok(vec![]);
        }
        s.split(',')
            .map(|s| T::from_env_var(s))
            .collect::<Result<Vec<_>, _>>()
            .map_err(FromEnvErr::from)
    }
}

macro_rules! impl_for_parseable {
    ($($t:ty),*) => {
        $(
            impl FromEnvVar for $t {
                type Error = <$t as FromStr>::Err;

                fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
                    parse_env_if_present(env_var)
                }
            }
        )*
    }
}

impl_for_parseable!(
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    url::Url,
    tracing::Level
);

#[cfg(feature = "alloy")]
impl_for_parseable!(
    alloy::primitives::Address,
    alloy::primitives::Bytes,
    alloy::primitives::U256
);

#[cfg(feature = "alloy")]
impl<const N: usize> FromEnvVar for alloy::primitives::FixedBytes<N> {
    type Error = <alloy::primitives::FixedBytes<N> as FromStr>::Err;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        parse_env_if_present(env_var)
    }
}

impl FromEnvVar for bool {
    type Error = std::str::ParseBoolError;

    fn from_env_var(env_var: &str) -> Result<Self, FromEnvErr<Self::Error>> {
        let s: String = std::env::var(env_var).map_err(|e| FromEnvErr::env_err(env_var, e))?;
        Ok(!s.is_empty())
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;

    fn set<T>(env: &str, val: &T)
    where
        T: ToString,
    {
        std::env::set_var(env, val.to_string());
    }

    fn load_expect_err<T>(env: &str, err: FromEnvErr<T::Error>)
    where
        T: FromEnvVar,
        T::Error: PartialEq,
    {
        let res = T::from_env_var(env).unwrap_err();
        assert_eq!(res, err);
    }

    fn test<T>(env: &str, val: T)
    where
        T: ToString + FromEnvVar + PartialEq + std::fmt::Debug,
    {
        set(env, &val);

        let res = T::from_env_var(env).unwrap();
        assert_eq!(res, val);
    }

    fn test_expect_err<T, U>(env: &str, value: U, err: FromEnvErr<T::Error>)
    where
        T: FromEnvVar,
        U: ToString,
        T::Error: PartialEq,
    {
        set(env, &value);
        load_expect_err::<T>(env, err);
    }

    #[test]
    fn test_primitives() {
        test("U8", 42u8);
        test("U16", 42u16);
        test("U32", 42u32);
        test("U64", 42u64);
        test("U128", 42u128);
        test("Usize", 42usize);
        test("I8", 42i8);
        test("I8-NEG", -42i16);
        test("I16", 42i16);
        test("I32", 42i32);
        test("I64", 42i64);
        test("I128", 42i128);
        test("Isize", 42isize);
        test("String", "hello".to_string());
        test("Url", url::Url::parse("http://example.com").unwrap());
        test("Level", tracing::Level::INFO);
    }

    #[test]
    fn test_duration() {
        let amnt = 42;
        let val = Duration::from_millis(42);

        set("Duration", &amnt);
        let res = Duration::from_env_var("Duration").unwrap();

        assert_eq!(res, val);
    }

    #[test]
    fn test_a_few_errors() {
        test_expect_err::<u8, _>(
            "U8_",
            30000u16,
            FromEnvErr::parse_error("30000".parse::<u8>().unwrap_err()),
        );

        test_expect_err::<u8, _>("U8_", "", FromEnvErr::empty("U8_"));
    }
}
