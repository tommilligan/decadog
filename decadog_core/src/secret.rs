/// Special support for secret values.
use std::fmt;

use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};

/// A secret string that should never be shown.
///
/// ```
/// # use decadog_core::secret::Secret;
/// #
/// let my_secret: Secret = "secret_value".to_owned().into();
///
/// // Secrets prevent accidental information leaks.
/// assert_eq!(format!("{}", my_secret), "sec***");
///
/// // But they can be used easily
/// assert_eq!(my_secret.as_ref(), "secret_value");
/// ```
#[derive(Clone, PartialEq)]
pub struct Secret {
    value: String,
}

impl Secret {
    fn new(secret: String) -> Self {
        Secret { value: secret }
    }

    fn hint(&self) -> &str {
        &self.value[..3]
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}***", self.hint())
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret {{ value: {}*** }}", self.hint())
    }
}

impl From<String> for Secret {
    fn from(secret: String) -> Self {
        Secret::new(secret)
    }
}

impl AsRef<str> for Secret {
    fn as_ref(&self) -> &str {
        self.value()
    }
}

struct SecretVisitor;

impl<'de> Visitor<'de> for SecretVisitor {
    type Value = Secret;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(value.to_owned().into())
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D>(deserializer: D) -> Result<Secret, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(SecretVisitor)
    }
}

impl Serialize for Secret {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.value())
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use serde_test::{assert_tokens, Token};

    use super::*;

    #[test]
    fn test_ser_de() {
        let secret = Secret::new("secret_value".to_owned());
        assert_tokens(&secret, &[Token::Str("secret_value")]);
    }

    #[test]
    fn test_format() {
        let secret = Secret::new("secret_value".to_owned());
        assert_eq!(format!("{}", &secret), "sec***");
        assert_eq!(format!("{:?}", &secret), "Secret { value: sec*** }");
    }
}
