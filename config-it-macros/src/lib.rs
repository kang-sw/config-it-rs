//!
//!
//! # Usage
//!
//! # Attributes
//!
//! All attributes are put inside of `#[config(...)]` or `#[config_it(...)]` attribute.
//!
//! - `alias = "<name>"`: Set alias for the field.
//! - `default = <expr>`: Set default value for the field.
//! - `default_expr = "<expr>"`: Set default value for the field.
//! - `admin | admin_write | admin_read`: Prohibit access to the field for the user. - `min = <expr>
//! | max = <expr> | one_of = [<expr>...]`: Sets constraint for the field
//! - `env = "<literal>"`: Sets environment variable name for the field.
//! - `transient | no_export | no_import`: Prohibit export/import of the field.
//! - `editor = $this::MetadataEditorHint::<ident>`: Sets editor hint for the field.
//! - `editor_hint = "<literal>"`: Sets editor hint for the field. Parses snake case string.
//! - `hide`: Hide field from the editor.
//!
//! # Using with non-config-it types
//!
//! For types which are not part of configuration, but does not provides `Default` trait, you can
//! use `#[config_it_default_expr = "<expr>"]` attribute to provide default for these types.
//!

use proc_macro::TokenStream;

#[proc_macro_derive(Template, attributes(config_it, config, config_it_default_expr))]
pub fn derive_collect_fn(item: TokenStream) -> TokenStream {
    Default::default()
}
