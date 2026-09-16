//! Validation for project names, crate names, and output paths.

use std::error::Error;
use std::fmt::{self, Display};
use std::path::{Component, Path, PathBuf, Prefix};

const MAX_PACKAGE_NAME_LEN: usize = 64;

const WINDOWS_RESERVED_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

/// A validated request to create a project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedProject {
    name: ProjectName,
    package_name: PackageName,
    crate_name: CrateName,
    output_root: OutputRoot,
    project_dir: PathBuf,
}

impl ValidatedProject {
    /// Validates and normalizes a create-project request.
    pub fn new(name: &str, output_root: impl AsRef<Path>, force: bool) -> ValidationResult<Self> {
        let name = ProjectName::parse(name)?;
        let package_name = PackageName::parse(&name.to_package_name())?;
        let crate_name = CrateName::from_package_name(&package_name)?;
        let output_root = OutputRoot::parse(output_root)?;
        let project_dir = output_root.path().join(package_name.as_str());

        validate_project_dir_availability(&project_dir, force)?;

        Ok(Self {
            name,
            package_name,
            crate_name,
            output_root,
            project_dir,
        })
    }

    /// Human-facing project name.
    #[must_use]
    pub fn name(&self) -> &ProjectName {
        &self.name
    }

    /// Cargo package name.
    #[must_use]
    pub fn package_name(&self) -> &PackageName {
        &self.package_name
    }

    /// Rust crate name used in generated code.
    #[must_use]
    pub fn crate_name(&self) -> &CrateName {
        &self.crate_name
    }

    /// Selected output root.
    #[must_use]
    pub fn output_root(&self) -> &OutputRoot {
        &self.output_root
    }

    /// Full generated project directory.
    #[must_use]
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }
}

/// A human-facing project name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectName(String);

impl ProjectName {
    /// Parses a human-facing project name.
    pub fn parse(value: &str) -> ValidationResult<Self> {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::new(
                ValidationErrorKind::EmptyProjectName,
                "project name cannot be empty",
            ));
        }

        if trimmed.len() > MAX_PACKAGE_NAME_LEN {
            return Err(ValidationError::new(
                ValidationErrorKind::ProjectNameTooLong,
                "project name must be 64 bytes or fewer",
            ));
        }

        if trimmed.starts_with(['-', '_', ' ']) || trimmed.ends_with(['-', '_', ' ']) {
            return Err(ValidationError::new(
                ValidationErrorKind::InvalidProjectName,
                "project name cannot start or end with a separator",
            ));
        }

        if !trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b' '))
        {
            return Err(ValidationError::new(
                ValidationErrorKind::InvalidProjectName,
                "project name may only contain ASCII letters, numbers, spaces, hyphens, and underscores",
            ));
        }

        let package_name = Self(trimmed.to_owned()).to_package_name();
        PackageName::parse(&package_name)?;

        Ok(Self(trimmed.to_owned()))
    }

    /// Returns the original human-facing project name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts the project name into a Cargo package name.
    #[must_use]
    pub fn to_package_name(&self) -> String {
        normalize_to_package_name(&self.0)
    }
}

/// A validated Cargo package name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageName(String);

impl PackageName {
    /// Parses a Cargo package name using Zoid's crates.io-style restrictions.
    pub fn parse(value: &str) -> ValidationResult<Self> {
        if value.is_empty() {
            return Err(ValidationError::new(
                ValidationErrorKind::EmptyPackageName,
                "package name cannot be empty",
            ));
        }

        if value.len() > MAX_PACKAGE_NAME_LEN {
            return Err(ValidationError::new(
                ValidationErrorKind::PackageNameTooLong,
                "package name must be 64 bytes or fewer",
            ));
        }

        if !value.is_ascii() {
            return Err(ValidationError::new(
                ValidationErrorKind::InvalidPackageName,
                "package name must be ASCII",
            ));
        }

        let first = value.as_bytes()[0];
        if !first.is_ascii_lowercase() {
            return Err(ValidationError::new(
                ValidationErrorKind::InvalidPackageName,
                "package name must start with a lowercase ASCII letter",
            ));
        }

        if !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        }) {
            return Err(ValidationError::new(
                ValidationErrorKind::InvalidPackageName,
                "package name may only contain lowercase ASCII letters, numbers, hyphens, and underscores",
            ));
        }

        if value.ends_with(['-', '_']) || value.contains("--") || value.contains("__") {
            return Err(ValidationError::new(
                ValidationErrorKind::InvalidPackageName,
                "package name cannot end with a separator or contain repeated separators",
            ));
        }

        if is_reserved_name(value) {
            return Err(ValidationError::new(
                ValidationErrorKind::ReservedName,
                "package name is reserved",
            ));
        }

        Ok(Self(value.to_owned()))
    }

    /// Returns the package name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated Rust crate name.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrateName(String);

impl CrateName {
    /// Derives a Rust crate name from a package name.
    pub fn from_package_name(package_name: &PackageName) -> ValidationResult<Self> {
        let crate_name = package_name.as_str().replace('-', "_");

        if RUST_KEYWORDS.contains(&crate_name.as_str()) {
            return Err(ValidationError::new(
                ValidationErrorKind::RustKeyword,
                "crate name cannot be a Rust keyword",
            ));
        }

        Ok(Self(crate_name))
    }

    /// Returns the crate name as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated output root path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OutputRoot(PathBuf);

impl OutputRoot {
    /// Parses an output root path.
    pub fn parse(path: impl AsRef<Path>) -> ValidationResult<Self> {
        let path = path.as_ref();

        if path.as_os_str().is_empty() {
            return Err(ValidationError::new(
                ValidationErrorKind::EmptyOutputPath,
                "output path cannot be empty",
            ));
        }

        for component in path.components() {
            match component {
                Component::ParentDir => {
                    return Err(ValidationError::new(
                        ValidationErrorKind::OutputPathTraversal,
                        "output path cannot contain parent directory components",
                    ));
                }
                Component::Prefix(p) => {
                    // Allow plain drive-letter prefixes (e.g. `C:`) so Windows
                    // absolute paths work.  Reject UNC, verbatim, and device
                    // prefixes — those are either network paths or unusual
                    // Windows forms that we don't need to support.
                    if !matches!(p.kind(), Prefix::Disk(_)) {
                        return Err(ValidationError::new(
                            ValidationErrorKind::UnsupportedOutputPath,
                            "only plain drive-letter paths are supported on Windows (no UNC or verbatim prefixes)",
                        ));
                    }
                }
                Component::Normal(part) if part.is_empty() => {
                    return Err(ValidationError::new(
                        ValidationErrorKind::UnsupportedOutputPath,
                        "output path contains an empty component",
                    ));
                }
                Component::RootDir | Component::CurDir | Component::Normal(_) => {}
            }
        }

        if path.exists() {
            let metadata = path.symlink_metadata().map_err(|error| {
                ValidationError::with_source(
                    ValidationErrorKind::OutputPathIo,
                    "failed to inspect output path",
                    error,
                )
            })?;

            if metadata.file_type().is_symlink() {
                return Err(ValidationError::new(
                    ValidationErrorKind::OutputPathSymlink,
                    "output path cannot be a symlink",
                ));
            }

            if !metadata.is_dir() {
                return Err(ValidationError::new(
                    ValidationErrorKind::OutputPathNotDirectory,
                    "output path exists but is not a directory",
                ));
            }
        }

        Ok(Self(path.to_path_buf()))
    }

    /// Returns the output root path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Validation result alias.
pub type ValidationResult<T> = Result<T, ValidationError>;

/// A validation failure.
#[derive(Debug)]
pub struct ValidationError {
    kind: ValidationErrorKind,
    message: String,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl ValidationError {
    fn new(kind: ValidationErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    fn with_source(
        kind: ValidationErrorKind,
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Returns the machine-readable validation error kind.
    #[must_use]
    pub fn kind(&self) -> ValidationErrorKind {
        self.kind
    }
}

impl Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ValidationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}

/// Machine-readable validation error categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidationErrorKind {
    /// The project name was empty.
    EmptyProjectName,
    /// The project name exceeded the maximum length.
    ProjectNameTooLong,
    /// The project name used unsupported syntax.
    InvalidProjectName,
    /// The normalized package name was empty.
    EmptyPackageName,
    /// The package name exceeded the maximum length.
    PackageNameTooLong,
    /// The package name used unsupported syntax.
    InvalidPackageName,
    /// The package or crate name is reserved.
    ReservedName,
    /// The crate name would be a Rust keyword.
    RustKeyword,
    /// The output path was empty.
    EmptyOutputPath,
    /// The output path attempted parent-directory traversal.
    OutputPathTraversal,
    /// The output path exists but is not a directory.
    OutputPathNotDirectory,
    /// The output path is a symlink.
    OutputPathSymlink,
    /// The output path could not be inspected.
    OutputPathIo,
    /// The output path used unsupported syntax.
    UnsupportedOutputPath,
    /// The generated project directory already exists.
    ProjectDirExists,
    /// The generated project directory exists as a symlink.
    ProjectDirSymlink,
    /// The generated project path could not be inspected.
    ProjectDirIo,
}

fn validate_project_dir_availability(project_dir: &Path, force: bool) -> ValidationResult<()> {
    if !project_dir.exists() {
        return Ok(());
    }

    let metadata = project_dir.symlink_metadata().map_err(|error| {
        ValidationError::with_source(
            ValidationErrorKind::ProjectDirIo,
            "failed to inspect generated project directory",
            error,
        )
    })?;

    if metadata.file_type().is_symlink() {
        return Err(ValidationError::new(
            ValidationErrorKind::ProjectDirSymlink,
            "generated project directory cannot be a symlink",
        ));
    }

    if force && metadata.is_dir() {
        return Ok(());
    }

    Err(ValidationError::new(
        ValidationErrorKind::ProjectDirExists,
        "generated project directory already exists; pass --force to allow reuse",
    ))
}

fn normalize_to_package_name(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = false;

    for (index, byte) in value.bytes().enumerate() {
        if byte.is_ascii_uppercase() {
            if index > 0 && !previous_was_separator {
                normalized.push('-');
            }
            normalized.push(byte.to_ascii_lowercase() as char);
            previous_was_separator = false;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            normalized.push(byte as char);
            previous_was_separator = false;
        } else if matches!(byte, b'-' | b'_' | b' ')
            && !previous_was_separator
            && !normalized.is_empty()
        {
            normalized.push('-');
            previous_was_separator = true;
        }
    }

    // Strip leading/trailing separators: `---hello` → `hello` (intentional —
    // the caller normalises user-provided crate names, and non-alphanumeric
    // prefixes have no semantic meaning in package identifiers).
    normalized.trim_matches('-').to_owned()
}

fn is_reserved_name(value: &str) -> bool {
    let canonical = value.replace('-', "_").to_ascii_lowercase();

    WINDOWS_RESERVED_NAMES.contains(&canonical.as_str())
        || RUST_KEYWORDS.contains(&canonical.as_str())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn project_name_normalizes_to_package_and_crate_names() {
        let project = ValidatedProject::new("HelloZoid", ".", false).expect("valid project");

        assert_eq!(project.name().as_str(), "HelloZoid");
        assert_eq!(project.package_name().as_str(), "hello-zoid");
        assert_eq!(project.crate_name().as_str(), "hello_zoid");
        assert_eq!(project.output_root().path(), Path::new("."));
        assert_eq!(project.project_dir(), Path::new("./hello-zoid"));
    }

    #[test]
    fn project_name_accepts_words_and_separators() {
        let name = ProjectName::parse("hello zoid_app").expect("valid project name");

        assert_eq!(name.to_package_name(), "hello-zoid-app");
    }

    #[test]
    fn project_name_rejects_empty_values() {
        let error = ProjectName::parse(" ").expect_err("empty name should fail");

        assert_eq!(error.kind(), ValidationErrorKind::EmptyProjectName);
    }

    #[test]
    fn project_name_rejects_path_like_values() {
        let error = ProjectName::parse("../zoid").expect_err("path-like name should fail");

        assert_eq!(error.kind(), ValidationErrorKind::InvalidProjectName);
    }

    #[test]
    fn project_name_rejects_reserved_package_names() {
        let error = ProjectName::parse("con").expect_err("reserved name should fail");

        assert_eq!(error.kind(), ValidationErrorKind::ReservedName);
    }

    #[test]
    fn package_name_rejects_uppercase_values() {
        let error = PackageName::parse("HelloZoid").expect_err("uppercase package should fail");

        assert_eq!(error.kind(), ValidationErrorKind::InvalidPackageName);
    }

    #[test]
    fn package_name_rejects_digit_first_values() {
        let error = PackageName::parse("1zoid").expect_err("digit-first package should fail");

        assert_eq!(error.kind(), ValidationErrorKind::InvalidPackageName);
    }

    #[test]
    fn output_root_allows_absolute_paths() {
        let root = OutputRoot::parse(std::env::temp_dir()).expect("absolute temp dir should pass");

        assert!(root.path().is_absolute());
    }

    #[test]
    fn output_root_rejects_parent_traversal() {
        let error = OutputRoot::parse("../other").expect_err("traversal should fail");

        assert_eq!(error.kind(), ValidationErrorKind::OutputPathTraversal);
    }

    #[test]
    fn output_root_rejects_existing_files() {
        let file = temp_path("zoid-output-file");
        fs::write(&file, "not a directory").expect("write temp file");

        let error = OutputRoot::parse(&file).expect_err("file output root should fail");

        assert_eq!(error.kind(), ValidationErrorKind::OutputPathNotDirectory);
        fs::remove_file(file).expect("remove temp file");
    }

    #[test]
    fn project_dir_requires_force_when_it_exists() {
        let root = temp_path("zoid-existing-project-root");
        let project_dir = root.join("hello-zoid");
        fs::create_dir_all(&project_dir).expect("create existing project dir");

        let error =
            ValidatedProject::new("HelloZoid", &root, false).expect_err("existing dir should fail");

        assert_eq!(error.kind(), ValidationErrorKind::ProjectDirExists);

        let project =
            ValidatedProject::new("HelloZoid", &root, true).expect("force allows existing dir");
        assert_eq!(project.project_dir(), project_dir);

        fs::remove_dir_all(root).expect("remove temp root");
    }

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos();

        std::env::temp_dir().join(format!("{label}-{unique}"))
    }
}
