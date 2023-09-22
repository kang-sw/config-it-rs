use std::borrow::Cow;

bitflags::bitflags! {
    #[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Copy, PartialEq, Eq)]
    pub struct MetaFlag: u32 {
        /// Disable import from `import` operation
        const NO_IMPORT = 1 << 0;

        /// Disable export to `export` operation
        const NO_EXPORT = 1 << 1;

        /// Hint monitor that this variable should be hidden from user.
        const HIDDEN = 1 << 2;

        /// Hint monitor that this variable should only be read by admin.
        const ADMIN_READ = 1 << 3;

        /// Hint monitor that this variable should only be written by admin.
        const ADMIN_WRITE = 1 << 4 | Self::ADMIN_READ.bits();

        /// Hint monitor that this is admin-only variable.
        const ADMIN = Self::ADMIN_READ.bits() | Self::ADMIN_WRITE.bits();

        /// Hint monitor that this variable is transient, and should not be saved to storage.
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

/// Shared generic properties of this metadata entity.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MetadataProps {
    /// Identifier for this config entity.
    pub name: &'static str,

    /// Typename for this config entity.
    pub type_name: &'static str,

    ///
    pub flags: MetaFlag,

    /// Hint for monitoring editor. This is not directly used by this crate, but exists for hinting
    /// remote monitor how to edit this variable.
    pub editor_hint: Option<MetadataEditorHint>,

    /// Optional schema. Will be used by remote monitor to manage this variable.
    #[cfg(feature = "jsonschema")]
    pub schema: Option<crate::Schema>,

    /// Source variable name. Usually same as 'name' unless another name is specified for it.
    pub varname: &'static str,

    ///
    pub description: &'static str,

    /// Environment variable name
    pub env: Option<&'static str>,
}
