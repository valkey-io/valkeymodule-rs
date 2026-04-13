use std::convert::TryFrom;

use crate::rediserror::ValkeyError;
use crate::ValkeyString;

/// Mockable interface for [`ValkeyString`].
///
/// This trait mirrors the `ValkeyString` read/parse methods so that module
/// command handlers can be unit-tested without a running Valkey server.
///
/// Methods return owned types (`String`, `Vec<u8>`) rather than references
/// so that mockall can auto-generate the mock implementation.
///
/// Production code can accept `&impl ValkeyStringInterface` or use generics
/// instead of a concrete [`ValkeyString`] to keep command logic unit-testable.
///
/// The corresponding `MockValkeyStringInterface` type is only exported when
/// compiling tests or when the crate is built with the `test-mocks` feature.
///
/// # Examples
///
/// ```rust,ignore
/// use valkey_module::{ValkeyStringInterface, MockValkeyString, ValkeyResult, ValkeyValue};
///
/// fn greet(name: &impl ValkeyStringInterface) -> ValkeyResult {
///     let name_str = name.try_as_str()?;
///     Ok(ValkeyValue::SimpleString(format!("Hello, {name_str}!")))
/// }
///
/// #[cfg(test)]
/// mod tests {
///     use super::*;
///
///     #[test]
///     fn test_greet() {
///         let mut mock = MockValkeyString::new();
///         mock.expect_try_as_str()
///             .returning(|| Ok("Alice".to_string()));
///         let result = greet(&mock).unwrap();
///     }
/// }
/// ```
#[cfg_attr(any(test, feature = "test-mocks"), mockall::automock)]
pub trait ValkeyStringInterface {
    fn try_as_str(&self) -> Result<String, ValkeyError>;
    fn as_slice(&self) -> Vec<u8>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn to_string_lossy(&self) -> String;
    fn parse_integer(&self) -> Result<i64, ValkeyError>;
    fn parse_float(&self) -> Result<f64, ValkeyError>;
    fn parse_unsigned_integer(&self) -> Result<u64, ValkeyError> {
        let val = self.parse_integer()?;
        u64::try_from(val)
            .map_err(|_| ValkeyError::Str("Couldn't parse negative number as unsigned integer"))
    }
}

impl ValkeyStringInterface for ValkeyString {
    fn try_as_str(&self) -> Result<String, ValkeyError> {
        ValkeyString::try_as_str(self).map(|s| s.to_string())
    }

    fn as_slice(&self) -> Vec<u8> {
        ValkeyString::as_slice(self).to_vec()
    }

    fn len(&self) -> usize {
        ValkeyString::len(self)
    }

    fn to_string_lossy(&self) -> String {
        ValkeyString::to_string_lossy(self)
    }

    fn parse_integer(&self) -> Result<i64, ValkeyError> {
        ValkeyString::parse_integer(self)
    }

    fn parse_float(&self) -> Result<f64, ValkeyError> {
        ValkeyString::parse_float(self)
    }
}

#[cfg(any(test, feature = "test-mocks"))]
pub use MockValkeyStringInterface as MockValkeyString;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_try_as_str() {
        let mut mock = MockValkeyString::new();
        mock.expect_try_as_str()
            .returning(|| Ok("hello".to_string()));
        assert_eq!(mock.try_as_str().unwrap(), "hello");
    }

    #[test]
    fn mock_as_slice() {
        let mut mock = MockValkeyString::new();
        mock.expect_as_slice().returning(|| b"hello".to_vec());
        assert_eq!(mock.as_slice(), b"hello");
    }

    #[test]
    fn mock_len() {
        let mut mock = MockValkeyString::new();
        mock.expect_len().returning(|| 5);
        assert_eq!(mock.len(), 5);
    }

    #[test]
    fn mock_is_empty() {
        let mut mock = MockValkeyString::new();
        mock.expect_is_empty().returning(|| true);
        assert!(mock.is_empty());

        let mut mock2 = MockValkeyString::new();
        mock2.expect_is_empty().returning(|| false);
        assert!(!mock2.is_empty());
    }

    #[test]
    fn mock_to_string_lossy() {
        let mut mock = MockValkeyString::new();
        mock.expect_to_string_lossy()
            .returning(|| "world".to_string());
        assert_eq!(mock.to_string_lossy(), "world");
    }

    #[test]
    fn mock_parse_integer() {
        let mut mock = MockValkeyString::new();
        mock.expect_parse_integer().returning(|| Ok(42));
        assert_eq!(mock.parse_integer().unwrap(), 42);
    }

    #[test]
    fn mock_parse_integer_error() {
        let mut mock = MockValkeyString::new();
        mock.expect_parse_integer()
            .returning(|| Err(ValkeyError::Str("Couldn't parse as integer")));
        assert!(mock.parse_integer().is_err());
    }

    #[test]
    fn mock_parse_unsigned_integer() {
        let mut mock = MockValkeyString::new();
        mock.expect_parse_unsigned_integer().returning(|| Ok(100));
        assert_eq!(mock.parse_unsigned_integer().unwrap(), 100);
    }

    #[test]
    fn mock_parse_unsigned_integer_negative() {
        let mut mock = MockValkeyString::new();
        mock.expect_parse_unsigned_integer().returning(|| {
            Err(ValkeyError::Str(
                "Couldn't parse negative number as unsigned integer",
            ))
        });
        assert!(mock.parse_unsigned_integer().is_err());
    }

    #[test]
    fn mock_parse_float() {
        let mut mock = MockValkeyString::new();
        mock.expect_parse_float().returning(|| Ok(3.14));
        let val = mock.parse_float().unwrap();
        assert!((val - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn mock_used_with_trait_bound() {
        fn get_name(s: &impl ValkeyStringInterface) -> String {
            s.try_as_str().unwrap().to_uppercase()
        }
        let mut mock = MockValkeyString::new();
        mock.expect_try_as_str()
            .returning(|| Ok("alice".to_string()));
        assert_eq!(get_name(&mock), "ALICE");
    }

    #[test]
    fn mock_with_times() {
        let mut mock = MockValkeyString::new();
        mock.expect_try_as_str()
            .times(2)
            .returning(|| Ok("test".to_string()));
        let _ = mock.try_as_str();
        let _ = mock.try_as_str();
    }
}
