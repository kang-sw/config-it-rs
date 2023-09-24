use std::{cell::Cell, mem::take};

use compact_str::{CompactString, ToCompactString};
use serde::{ser::SerializeMap, Deserialize, Serialize};

#[cfg(not(feature = "indexmap"))]
type Map<T, V> = std::collections::BTreeMap<T, V>;

#[cfg(feature = "indexmap")]
type Map<T, V> = indexmap::IndexMap<T, V>;

/// Defines rules for serializing category names within the [`Archive`].
///
/// When an [`Archive`] is serialized, it manifests as a map of key-value pairs, where the key is a
/// string, and the value is a plain object. Due to the indistinguishable nature of category keys
/// from value object keys, a unique naming convention for categories is necessary.
///
/// The default naming convention prefixes category names with `~`. For instance, a category named
/// `hello` is serialized as `~hello`.
///
/// Users can customize this rule for both serialization and deserialization processes by invoking
/// the [`with_category_rule`] function.
pub enum CategoryRule<'a> {
    /// Categories are signified by prefixing their names with the specified token.
    Prefix(&'a str),

    /// Categories are signified by appending their names with the specified token.
    Suffix(&'a str),

    /// Categories are signified by wrapping their names between the specified start and end tokens.
    Wrap(&'a str, &'a str),
}

thread_local! {
    static CATEGORY_RULE: Cell<CategoryRule<'static>> = Cell::new(Default::default());
}

/// Temporarily overrides the category serialization rule while serializing or deserializing a map.
///
/// This function allows for a custom `CategoryRule` to be specified for the duration of the
/// provided closure, `f`. This is especially useful when there's a need to diverge from the default
/// category naming convention during a specific serialization or deserialization task.
///
/// # Safety
///
/// The implementation uses thread-local storage (`CATEGORY_RULE`) to store the custom rule. The
/// original rule or default value is safely restored upon function exit, ensuring consistent
/// behavior across different invocations. Moreover, this function is panic-safe, meaning if a panic
/// occurs within the closure, the rule is still guaranteed to be restored before the panic
/// propagates.
///
/// # Arguments
///
/// * `rule`: The custom category rule to apply.
/// * `f`: A closure representing the serialization or deserialization task where the custom rule
///   should be used.
///
/// # Panics
///
/// This function will propagate any panics that occur within the provided closure.
pub fn with_category_rule(rule: CategoryRule, f: impl FnOnce() + std::panic::UnwindSafe) {
    CATEGORY_RULE.with(|x| unsafe {
        // SAFETY: Temporarily override lifetime as &'static; The `x` is guaranteed to be restored
        //         to its original value on function exit, even if a panic occurs.
        x.replace(std::mem::transmute(rule));

        let err = std::panic::catch_unwind(|| {
            f();
        });

        x.replace(Default::default());

        // Let panic propagate
        err.unwrap();
    })
}

impl<'a> Default for CategoryRule<'a> {
    fn default() -> Self {
        Self::Prefix("~")
    }
}

impl<'a> CategoryRule<'a> {
    pub fn is_category(&self, key: &str) -> bool {
        match self {
            Self::Prefix(prefix) => key.starts_with(prefix),
            Self::Suffix(suffix) => key.ends_with(suffix),
            Self::Wrap(prefix, suffix) => key.starts_with(prefix) && key.ends_with(suffix),
        }
    }

    pub fn make_category(&self, key: &str, out_key: &mut CompactString) {
        out_key.clear();

        match self {
            CategoryRule::Prefix(tok) => {
                out_key.push_str(tok);
                out_key.push_str(key);
            }

            CategoryRule::Suffix(tok) => {
                out_key.push_str(key);
                out_key.push_str(tok);
            }

            CategoryRule::Wrap(pre, suf) => {
                out_key.push_str(pre);
                out_key.push_str(key);
                out_key.push_str(suf);
            }
        }
    }
}

/// Represents a hierarchical archive of configuration values and categories.
///
/// The `Archive` struct organizes configuration data into a tree-like structure. Each node in this
/// structure can represent either a configuration category (identified by a path) or an individual
/// configuration value.
///
/// Categories within the archive are uniquely identified by keys that are prefixed with a special
/// character, typically `~`, though this can be customized using [`with_category_rule`]. Each
/// category can further contain nested categories and values, allowing for a deeply nested
/// hierarchical organization of configuration data.
///
/// Configuration values within a category are stored as key-value pairs, where the key is a string
/// representing the configuration name, and the value is its associated JSON representation.
///
/// This structure provides a compact and efficient way to represent, serialize, and deserialize
/// configuration data with support for custom category naming conventions.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct Archive {
    /// Every '~' prefixed keys
    pub(crate) paths: Map<CompactString, Archive>,

    /// All elements except child path nodes.
    pub(crate) values: Map<CompactString, serde_json::Value>,
}

impl Archive {
    pub fn iter_values(&self) -> impl Iterator<Item = (&str, &serde_json::Value)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn iter_paths(&self) -> impl Iterator<Item = (&str, &Archive)> {
        self.paths.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn iter_paths_mut(&mut self) -> impl Iterator<Item = (&str, &mut Archive)> {
        self.paths.iter_mut().map(|(k, v)| (k.as_str(), v))
    }

    pub fn iter_values_mut(&mut self) -> impl Iterator<Item = (&str, &mut serde_json::Value)> {
        self.values.iter_mut().map(|(k, v)| (k.as_str(), v))
    }

    pub fn get_value(&self, key: &str) -> Option<&serde_json::Value> {
        self.values.get(key)
    }

    pub fn get_value_mut(&mut self, key: &str) -> Option<&mut serde_json::Value> {
        self.values.get_mut(key)
    }

    pub fn get_path(&self, key: &str) -> Option<&Archive> {
        self.paths.get(key)
    }

    pub fn get_path_mut(&mut self, key: &str) -> Option<&mut Archive> {
        self.paths.get_mut(key)
    }

    pub fn insert_value(&mut self, key: impl ToCompactString, value: serde_json::Value) {
        self.values.insert(key.to_compact_string(), value);
    }

    pub fn insert_path(&mut self, key: impl ToCompactString, value: Archive) {
        self.paths.insert(key.to_compact_string(), value);
    }

    pub fn remove_value(&mut self, key: &str) -> Option<serde_json::Value> {
        self.values.remove(key)
    }

    pub fn remove_path(&mut self, key: &str) -> Option<Archive> {
        self.paths.remove(key)
    }

    pub fn clear_values(&mut self) {
        self.values.clear();
    }

    pub fn clear_paths(&mut self) {
        self.paths.clear();
    }

    pub fn is_empty_values(&self) -> bool {
        self.values.is_empty()
    }

    pub fn is_empty_paths(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn len_values(&self) -> usize {
        self.values.len()
    }

    pub fn len_paths(&self) -> usize {
        self.paths.len()
    }
}

impl Archive {
    /// Searches for a nested category within the archive using the specified path.
    ///
    /// Given a path (as an iterator of strings), this method traverses the archive hierarchically
    /// to locate the target category.
    ///
    /// # Parameters
    /// * `path`: The path of the target category, represented as an iterable of strings.
    ///
    /// # Returns
    /// An `Option` containing a reference to the found `Archive` (category) if it exists,
    /// or `None` otherwise.
    pub fn find_path<'s, 'a, T: AsRef<str> + 'a>(
        &'s self,
        path: impl IntoIterator<Item = T>,
    ) -> Option<&'s Archive> {
        let iter = path.into_iter();
        let mut paths = &self.paths;
        let mut node = None;

        for key in iter {
            if let Some(next_node) = paths.get(key.as_ref()) {
                node = Some(next_node);
                paths = &next_node.paths;
            } else {
                return None;
            }
        }

        node
    }

    /// Retrieves a mutable reference to a nested category, creating it if it doesn't exist.
    ///
    /// This method is useful for ensuring a category exists at a certain path, creating any
    /// necessary intermediate categories along the way.
    ///
    /// # Parameters
    /// * `path`: The path of the target category, represented as an iterable of strings.
    ///
    /// # Returns
    /// A mutable reference to the target `Archive` (category).
    pub fn find_or_create_path_mut<'s, 'a>(
        &'s mut self,
        path: impl IntoIterator<Item = &'a str>,
    ) -> &'s mut Archive {
        path.into_iter().fold(self, |node, key| node.paths.entry(key.into()).or_default())
    }

    /// Generates a differential patch between the current and a newer archive.
    ///
    /// This method examines the differences between the current archive and a provided newer
    /// archive. The result is an `Archive` containing only the differences. The method also
    /// modifies the `newer` archive in place, removing the elements that are part of the patch.
    ///
    /// # Parameters
    /// * `newer`: A mutable reference to the newer version of the archive.
    ///
    /// # Returns
    /// An `Archive` containing only the differences between the current and newer archives.
    pub fn create_patch(&self, newer: &mut Self) -> Self {
        let mut patch = Self::default();

        newer.paths.retain(|k, v| {
            if let Some(base_v) = self.paths.get(k) {
                let patch_v = base_v.create_patch(v);
                if !patch_v.is_empty() {
                    patch.paths.insert(k.clone(), patch_v);
                    !v.is_empty()
                } else {
                    true
                }
            } else {
                patch.paths.insert(k.clone(), take(v));
                false
            }
        });

        newer.values.retain(|k, v| {
            if let Some(base_v) = self.values.get(k) {
                if *base_v != *v {
                    patch.values.insert(k.clone(), take(v));
                    false
                } else {
                    true
                }
            } else {
                patch.values.insert(k.clone(), take(v));
                false
            }
        });

        patch
    }

    /// Checks if the archive is empty.
    ///
    /// An archive is considered empty if it has no categories (paths) and no values.
    ///
    /// # Returns
    /// `true` if the archive is empty, otherwise `false`.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.values.is_empty()
    }

    /// Merges data from another archive into the current one.
    ///
    /// This method recursively merges categories and replaces values from the other archive into
    /// the current one. In the case of overlapping categories, it will dive deeper and merge the
    /// inner values and categories.
    ///
    /// # Parameters
    /// * `other`: The other archive to merge from.
    pub fn merge_from(&mut self, other: Self) {
        // Recursively merge p
        for (k, v) in other.paths {
            self.paths.entry(k).or_default().merge_from(v);
        }

        // Value merge is done with simple replace
        for (k, v) in other.values {
            self.values.insert(k, v);
        }
    }

    /// Merges data from another archive into a clone of the current one and returns the merged
    /// result.
    ///
    /// This method is a combinatory operation that uses `merge_from` under the hood but doesn't
    /// modify the current archive in place, instead, it returns a new merged archive.
    ///
    /// # Parameters
    /// * `other`: The other archive to merge from.
    ///
    /// # Returns
    /// A new `Archive` which is the result of merging the current archive with the provided one.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.merge_from(other);
        self
    }
}

impl<'a> Deserialize<'a> for Archive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        struct PathNodeVisit {
            build: Archive,
        }

        impl<'de> serde::de::Visitor<'de> for PathNodeVisit {
            type Value = Archive;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Object consist of Tilde(~) prefixed objects or ")
            }

            fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                CATEGORY_RULE.with(|rule| {
                    let rule = rule.take();

                    while let Some(mut key) = map.next_key::<CompactString>()? {
                        if !key.is_empty() && rule.is_category(&key) {
                            key.remove(0); // Exclude initial tilde

                            let child: Archive = map.next_value()?;
                            self.build.paths.insert(key, child);
                        } else {
                            let value: serde_json::Value = map.next_value()?;
                            self.build.values.insert(key, value);
                        }
                    }

                    Ok(self.build)
                })
            }
        }

        deserializer.deserialize_map(PathNodeVisit { build: Default::default() })
    }
}

impl Serialize for Archive {
    fn serialize<S>(&self, se: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = se.serialize_map(Some(self.paths.len() + self.values.len()))?;

        CATEGORY_RULE.with(|rule| {
            let rule = rule.take();
            let mut key_b = CompactString::default();

            for (k, v) in &self.paths {
                rule.make_category(k, &mut key_b);
                map.serialize_entry(&key_b, v)?;
            }

            Ok(())
        })?;

        for (k, v) in &self.values {
            debug_assert!(
                !k.starts_with('~'),
                "Tilde prefixed key '{k}' for field is not allowed!"
            );

            map.serialize_entry(k, v)?;
        }

        map.end()
    }
}

#[test]
#[allow(clippy::approx_constant)]
fn test_archive_basic() {
    let src = r#"
        {
            "~root_path_1": {
                "~subpath1": {
                    "value1": null,
                    "value2": {},
                    "~sub-subpath": {}
                },
                "~subpath2": {}
            },
            "~root_path_2": {
                "value1": null,
                "value2": 31.4,
                "value3": "hoho-haha",
                "value-obj": {
                    "~pathlike": 3.141
                }
            }
        }
    "#;

    let arch: Archive = serde_json::from_str(src).unwrap();
    assert!(arch.paths.len() == 2);

    let p1 = arch.paths.get("root_path_1").unwrap();
    assert!(p1.paths.len() == 2);
    assert!(p1.values.is_empty());

    let sp1 = p1.paths.get("subpath1").unwrap();
    assert!(sp1.paths.contains_key("sub-subpath"));
    assert!(sp1.values.len() == 2);
    assert!(sp1.values.contains_key("value1"));
    assert!(sp1.values.contains_key("value2"));
    assert!(sp1.values.get("value1").unwrap().is_null());
    assert!(sp1.values.get("value2").unwrap().as_object().unwrap().is_empty());

    let p2 = arch.paths.get("root_path_2").unwrap();
    assert!(p2.paths.is_empty());
    assert!(p2.values.len() == 4);

    let newer = r#"
        {
            "~root_path_1": {
                "~subpath1": {
                    "value1": null,
                    "value2": {
                        "hello, world!": 3.141
                    },
                    "~sub-subpath": {}
                },
                "~subpath2": {},
                "~new_path": {
                    "valll": 4.44
                }
            },
            "~root_path_2": {
                "value1": null,
                "value2": 31.4,
                "value3": "hoho-haha",
                "value-obj": {
                    "~pathlike": 3.141
                }
            }
        }
    "#;
    let newer: Archive = serde_json::from_str(newer).unwrap();
    let mut newer_consume = newer.clone();
    let patch = Archive::create_patch(&arch, &mut newer_consume);

    let merged = arch.clone().merge(patch.clone());
    assert_eq!(merged, newer);

    assert!(patch.paths.len() == 1);
    assert!(patch.paths.contains_key("root_path_1"));

    let val = &patch.find_path(["root_path_1", "subpath1"]).unwrap().values;
    let val_obj = val.get("value2").unwrap().as_object().unwrap();
    assert!(val.contains_key("value2"));
    assert!(val_obj.len() == 1);
    assert!(val_obj.contains_key("hello, world!"));
    assert!(val_obj.get("hello, world!") == Some(&serde_json::Value::from(3.141)));

    let val = &patch.find_path(["root_path_1", "new_path"]).unwrap().values;
    assert!(val.contains_key("valll"));
    assert!(val.get("valll") == Some(&serde_json::Value::from(4.44)));
}
