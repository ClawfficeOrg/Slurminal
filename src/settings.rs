use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf, Prefix};

use gpui_kit::component::ThemeMode;

const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Persisted application settings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppSettings {
    /// Settings file schema version; must equal `CURRENT_SCHEMA_VERSION`.
    pub schema_version: u32,
    /// Appearance preferences.
    pub appearance: AppearanceSettings,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            appearance: AppearanceSettings::default(),
        }
    }
}

impl AppSettings {
    /// Loads settings from `path`.
    ///
    /// Returns `Ok(Default)` when the file is absent so callers can
    /// fall back to defaults without treating a missing file as an error.
    pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, SettingsError> {
        let path = path.as_ref();
        validate_settings_path(path)?;

        match fs::read_to_string(path) {
            Ok(source) => Self::parse(&source),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(SettingsError::Io {
                path: path.to_path_buf(),
                source: error,
            }),
        }
    }

    /// Saves settings to `path` using an atomic temp-file rename.
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<(), SettingsError> {
        let path = path.as_ref();
        validate_settings_path(path)?;
        self.validate()?;

        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| SettingsError::Io {
                path: parent.to_path_buf(),
                source: error,
            })?;
        }

        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, self.to_file_string()).map_err(|error| SettingsError::Io {
            path: temp_path.clone(),
            source: error,
        })?;
        fs::rename(&temp_path, path).map_err(|error| SettingsError::Io {
            path: path.to_path_buf(),
            source: error,
        })
    }

    /// Parses settings from a `key=value` formatted string.
    pub fn parse(source: &str) -> Result<Self, SettingsError> {
        let values = parse_key_values(source)?;
        let defaults = Self::default();

        let schema_version =
            parse_optional_u32(&values, "schema_version")?.unwrap_or(defaults.schema_version);

        let theme = values
            .get("theme")
            .and_then(|v| parse_theme_mode(v))
            .unwrap_or(defaults.appearance.theme);

        let settings = Self {
            schema_version,
            appearance: AppearanceSettings { theme },
        };
        settings.validate()?;
        Ok(settings)
    }

    /// Serializes settings to the `key=value` format written to disk.
    pub fn to_file_string(&self) -> String {
        format!(
            "schema_version={}\ntheme={}\n",
            self.schema_version,
            self.appearance.theme.name()
        )
    }

    /// Validates that stored field values are within accepted ranges.
    pub fn validate(&self) -> Result<(), SettingsError> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(SettingsError::Validation(format!(
                "unsupported settings schema version {}",
                self.schema_version
            )));
        }
        Ok(())
    }
}

/// Parses the settings file's theme value into a [`ThemeMode`].
///
/// `ThemeMode::name` provides the serialised form (`"light"` / `"dark"`), but
/// gpui-component has no matching parser, so the inverse lives here. Unknown
/// values return `None` and the caller falls back to the default rather than
/// failing the load — a settings file from a newer build should not stop the
/// app from starting.
fn parse_theme_mode(value: &str) -> Option<ThemeMode> {
    match value {
        "light" => Some(ThemeMode::Light),
        "dark" => Some(ThemeMode::Dark),
        _ => None,
    }
}

/// Appearance preferences stored in settings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppearanceSettings {
    /// Active theme mode.
    pub theme: ThemeMode,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
        }
    }
}

/// Errors that can occur during settings load, save, or validation.
#[derive(Debug)]
pub enum SettingsError {
    /// An I/O error occurred at the given path.
    Io { path: PathBuf, source: io::Error },
    /// The settings file contained an unrecognised line.
    Parse { line: usize, message: String },
    /// A parsed value failed domain validation.
    Validation(String),
    /// A path component would escape the expected directory.
    UnsafePath(PathBuf),
}

impl Display for SettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "settings I/O failed at `{}`: {source}",
                    path.display()
                )
            }
            Self::Parse { line, message } => {
                write!(formatter, "settings parse error on line {line}: {message}")
            }
            Self::Validation(message) => formatter.write_str(message),
            Self::UnsafePath(path) => {
                write!(formatter, "settings path `{}` is unsafe", path.display())
            }
        }
    }
}

impl Error for SettingsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { .. } | Self::Validation(_) | Self::UnsafePath(_) => None,
        }
    }
}

fn parse_key_values(source: &str) -> Result<BTreeMap<String, String>, SettingsError> {
    let mut values: BTreeMap<String, String> = BTreeMap::new();

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            return Err(SettingsError::Parse {
                line: line_number,
                message: "expected `key=value`".to_string(),
            });
        };

        let key = key.trim();
        let value = value.trim();

        if key.is_empty() {
            return Err(SettingsError::Parse {
                line: line_number,
                message: "settings key cannot be empty".to_string(),
            });
        }

        if values.insert(key.to_string(), value.to_string()).is_some() {
            return Err(SettingsError::Parse {
                line: line_number,
                message: format!("duplicate settings key `{key}`"),
            });
        }
    }

    for key in values.keys() {
        if !matches!(key.as_str(), "schema_version" | "theme") {
            return Err(SettingsError::Parse {
                line: 0,
                message: format!("unknown settings key `{key}`"),
            });
        }
    }

    Ok(values)
}

fn parse_optional_u32(
    values: &BTreeMap<String, String>,
    key: &str,
) -> Result<Option<u32>, SettingsError> {
    values
        .get(key)
        .map(|value| {
            value.parse::<u32>().map_err(|_| {
                SettingsError::Validation(format!("`{key}` must be an unsigned integer"))
            })
        })
        .transpose()
}

fn validate_settings_path(path: &Path) -> Result<(), SettingsError> {
    if path.as_os_str().is_empty() || path.file_name().is_none() {
        return Err(SettingsError::UnsafePath(path.to_path_buf()));
    }

    for component in path.components() {
        let bad = match component {
            Component::ParentDir => true,
            Component::Prefix(p) => !matches!(p.kind(), Prefix::Disk(_)),
            _ => false,
        };
        if bad {
            return Err(SettingsError::UnsafePath(path.to_path_buf()));
        }
    }

    Ok(())
}
