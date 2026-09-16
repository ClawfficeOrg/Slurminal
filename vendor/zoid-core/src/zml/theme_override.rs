//! ZML theme token override declarations.

use serde::{Deserialize, Serialize};

use crate::zml::error::{ZmlError, ZmlErrorKind, ZmlResult};
use crate::zml::prop::ZmlValue;

/// Maximum allowed depth for a theme override token path.
pub const MAX_THEME_PATH_DEPTH: usize = 8;

/// A design-token override applied within the scope of a ZML document.
///
/// The `path` is a dot-separated sequence of keys identifying the token to
/// override, split into components: `["color", "accent"]` overrides
/// `color.accent`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThemeOverride {
    /// Token path components. Must be non-empty and at most
    /// [`MAX_THEME_PATH_DEPTH`] elements long.
    pub path: Vec<String>,
    /// Replacement value for the token.
    pub value: ZmlValue,
}

impl ThemeOverride {
    /// Creates a new `ThemeOverride`, returning an error if `path` is empty or
    /// exceeds [`MAX_THEME_PATH_DEPTH`].
    pub fn new(path: Vec<String>, value: ZmlValue) -> ZmlResult<Self> {
        if path.is_empty() {
            return Err(ZmlError::new(
                ZmlErrorKind::ThemePathEmpty,
                "theme override path cannot be empty",
            ));
        }
        if path.len() > MAX_THEME_PATH_DEPTH {
            return Err(ZmlError::new(
                ZmlErrorKind::ThemePathTooDeep,
                format!(
                    "theme override path depth {} exceeds maximum of {}",
                    path.len(),
                    MAX_THEME_PATH_DEPTH
                ),
            ));
        }
        Ok(Self { path, value })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn roundtrip(t: &ThemeOverride) -> ThemeOverride {
        let s = toml::to_string(t).expect("serialize");
        toml::from_str(&s).expect("deserialize")
    }

    #[test]
    fn single_segment_path_roundtrips() {
        let t = ThemeOverride::new(
            vec!["background".to_owned()],
            ZmlValue::String("#1e1e2e".to_owned()),
        )
        .expect("valid");
        assert_eq!(roundtrip(&t), t);
    }

    #[test]
    fn multi_segment_path_roundtrips() {
        let t = ThemeOverride::new(
            vec!["color".to_owned(), "accent".to_owned()],
            ZmlValue::TokenRef("brand.primary".to_owned()),
        )
        .expect("valid");
        assert_eq!(roundtrip(&t), t);
    }

    #[test]
    fn empty_path_is_rejected() {
        let err =
            ThemeOverride::new(vec![], ZmlValue::Bool(true)).expect_err("empty path should fail");
        assert_eq!(err.kind(), ZmlErrorKind::ThemePathEmpty);
    }

    #[test]
    fn path_exceeding_max_depth_is_rejected() {
        let path = (0..=MAX_THEME_PATH_DEPTH)
            .map(|i| format!("seg{i}"))
            .collect();
        let err =
            ThemeOverride::new(path, ZmlValue::Bool(true)).expect_err("too-deep path should fail");
        assert_eq!(err.kind(), ZmlErrorKind::ThemePathTooDeep);
    }

    #[test]
    fn max_depth_path_is_accepted() {
        let path = (0..MAX_THEME_PATH_DEPTH)
            .map(|i| format!("seg{i}"))
            .collect();
        ThemeOverride::new(path, ZmlValue::Bool(true)).expect("max depth should be valid");
    }
}
