//! Backend-agnostic JSON entry point for the Nostr wire types.
//!
//! `nostro2` encodes its types with either `serde_json` or `json-bourne`,
//! selected by mutually exclusive cargo features. Every call site in this
//! crate goes through [`NostrJson`], so the `#[cfg]` split lives here only.

pub struct NostrJson;

impl NostrJson {
    /// Encode a Nostr wire type as a JSON string.
    ///
    /// # Errors
    /// Returns the active backend's JSON error.
    #[cfg(feature = "serde")]
    pub fn to_string<T>(value: &T) -> Result<String, nostro2::errors::NostrErrors>
    where
        T: serde::Serialize + ?Sized,
    {
        Ok(serde_json::to_string(value)?)
    }

    /// Encode a Nostr wire type as a JSON string.
    ///
    /// # Errors
    /// Returns the active backend's JSON error.
    #[cfg(feature = "bourne")]
    pub fn to_string<T>(value: &T) -> Result<String, nostro2::errors::NostrErrors>
    where
        T: json_bourne::ToJson + ?Sized,
    {
        Ok(json_bourne::to_string(value)?)
    }

    /// Parse a Nostr wire type from a JSON string.
    ///
    /// # Errors
    /// Returns the active backend's JSON error.
    #[cfg(feature = "serde")]
    pub fn parse_str<T>(input: &str) -> Result<T, nostro2::errors::NostrErrors>
    where
        T: serde::de::DeserializeOwned,
    {
        Ok(serde_json::from_str(input)?)
    }

    /// Parse a Nostr wire type from a JSON string.
    ///
    /// # Errors
    /// Returns the active backend's JSON error.
    #[cfg(feature = "bourne")]
    pub fn parse_str<T>(input: &str) -> Result<T, nostro2::errors::NostrErrors>
    where
        T: for<'a> json_bourne::FromJson<'a>,
    {
        Ok(json_bourne::parse_str(input)?)
    }
}
