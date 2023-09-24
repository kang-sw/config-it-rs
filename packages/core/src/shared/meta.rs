use std::borrow::Cow;

bitflags::bitflags! {
    /// Represents metadata flags for a configuration entity.
    ///
    /// These flags provide hints and directives to the config system (`config-it`)
    /// about how to treat and interact with the associated configuration variable.
    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
    pub struct MetaFlag: u32 {
        /// Prevents the variable from being included in `import` operations.
        const NO_IMPORT = 1 << 0;

        /// Prevents the variable from being included in `export` operations.
        const NO_EXPORT = 1 << 1;

        /// Advises the monitor to hide this variable from non-admin users.
        const HIDDEN_NON_ADMIN = 1 << 2;

        /// Advises the monitor to hide this variable from all users, regardless of role.
        const HIDDEN = 1 << 6;

        /// Indicates that only admin users should be allowed to read this variable.
        const ADMIN_READ = 1 << 3;

        /// Indicates that only admin users should be allowed to write to this variable. Implicitly
        /// includes the `ADMIN_READ` permission.
        const ADMIN_WRITE = 1 << 4 | Self::ADMIN_READ.bits();

        /// Designates the variable as read-only, ensuring that no user, even admins, can modify its
        /// value.
        const READONLY = 1 << 5;

        /// Marks the variable as write-only. This means its value cannot be read or accessed
        /// (useful for secrets, where the value might be set but not retrieved).
        const WRITEONLY = 1 << 7;

        /// Ensures that the variable's value is encrypted when stored. Implicitly includes the
        /// `WRITEONLY` property, reflecting the sensitive nature of the data.
        const SECRET = 1 << 8 | Self::WRITEONLY.bits();

        /// Designates the variable as exclusive to admin users for both reading and writing.
        const ADMIN = Self::ADMIN_READ.bits() | Self::ADMIN_WRITE.bits();

        /// Informs the monitor that this variable is transient, implying that it shouldn't be
        /// persisted to storage. This combines both `NO_EXPORT` and `NO_IMPORT` properties.
        const TRANSIENT = MetaFlag::NO_EXPORT.bits() | MetaFlag::NO_IMPORT.bits();
    }
}

/// Hint for backend editor. This is not used by config-it itself.
///
/// This is used by remote monitor to determine how to edit this variable.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataEditorHint {
    /// For color in range [0.0, 1.0]
    ///
    /// - [number; 3] -> RGB
    /// - [number; 4] -> RGBA
    ColorRgba255,

    /// For color in range [0, 255]
    ///
    /// - [number; 3] -> RGB
    /// - [number; 4] -> RGBA
    /// - string -> hex color
    /// - integer -> 32 bit hex color `[r,g,b,a] = [0,8,16,24].map(|x| 0xff & (color >> x))`
    ColorRgbaReal,

    /// Any string type will be treated as multiline text.
    MultilineText,

    /// Any string type will be treated as code, with given language hint.
    Code(Cow<'static, str>),
}

/// Describes metadata for a configuration entity, intended for utilization by external tools.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Metadata {
    /// Unique identifier for this configuration entity.
    pub name: &'static str,

    /// Source variable name from the configuration definition. It's typically identical to 'name'
    /// unless an alternative name is explicitly set.
    pub varname: &'static str,

    /// String representation of the type for this configuration entity.
    pub type_name: &'static str,

    /// Flags that denote various behaviors and hints associated with the configuration entity.
    pub flags: MetaFlag,

    /// Provides guidance for a monitoring editor on how to interact with this variable. While this
    /// crate doesn't use this hint directly, it helps external monitors understand the preferred
    /// method to edit the variable.
    pub editor_hint: Option<MetadataEditorHint>,

    /// If enabled, provides a schema that remote monitors can leverage to manage this variable
    /// effectively.
    #[cfg(feature = "jsonschema")]
    pub schema: Option<crate::Schema>,

    /// A brief description that elaborates on the purpose or role of the configuration entity.
    pub description: &'static str,

    /// Corresponding environment variable name, if any, that maps to this configuration entity.
    pub env: Option<&'static str>,
}
