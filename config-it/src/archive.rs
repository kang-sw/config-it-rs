use std::{cell::Cell, collections::BTreeMap, mem::take};

use compact_str::CompactString;
use serde::{ser::SerializeMap, Deserialize, Serialize};

type Map<T, V> = BTreeMap<T, V>;

///
/// [`Archive`] is serialized a map of key-value pairs, where key is a string, and value is
/// plain object. Since key for categories cannot be distinguished from value objects' keys,
/// a special rule for category name is applied.
///
/// Default rule is to prefix category name with `~`. So a category named `hello` will be
/// serialized as `~hello`.
///
/// This rule can be changed by serialize or deserialize objects within boundary of
/// [`with_category_rule`].
///
pub enum CategoryRule {
    /// Category name is prefixed with this token.
    Prefix(&'static str),

    /// Category name is suffixed with this token.
    Suffix(&'static str),

    /// Category name is wrapped with this token.
    Wrap(&'static str, &'static str),
}

thread_local! {
    static CATEGORY_RULE: Cell<CategoryRule> = Cell::new(Default::default());
}

///
/// Serialize or deserialize a map with customized category rule support.
///
pub fn with_category_rule(rule: CategoryRule, f: impl FnOnce()) {
    CATEGORY_RULE.with(|x| {
        x.replace(rule);
        f();
        x.replace(Default::default());
    })
}

impl Default for CategoryRule {
    fn default() -> Self {
        Self::Prefix("~")
    }
}

impl CategoryRule {
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

#[derive(Default, Clone, Debug, PartialEq)]
pub struct Archive {
    /// Every '~' prefixed keys
    pub paths: Map<CompactString, Archive>,

    /// All elements except child path nodes.
    pub values: Map<CompactString, serde_json::Value>,
}

impl Archive {
    pub fn find_path<'s, 'a>(
        &'s self,
        path: impl IntoIterator<Item = &'a str>,
    ) -> Option<&'s Archive> {
        let mut iter = path.into_iter();
        let mut paths = &self.paths;
        let mut node = None;

        while let Some(key) = iter.next() {
            if let Some(next_node) = paths.get(key) {
                node = Some(next_node);
                paths = &next_node.paths;
            } else {
                return None;
            }
        }

        node
    }

    pub fn find_or_create_path_mut<'s, 'a>(
        &'s mut self,
        path: impl IntoIterator<Item = &'a str>,
    ) -> &'s mut Archive {
        let mut iter = path.into_iter();

        let mut key = iter.next().unwrap();
        let mut node = self.paths.entry(key.into()).or_default();

        loop {
            if let Some(k) = iter.next() {
                key = k;
            } else {
                break;
            }

            node = node.paths.entry(key.into()).or_default();
        }

        node
    }

    ///
    /// Creates archive which contains only the differences between two archives.
    /// This does not affect to removed categories/values of newer archive.
    ///
    /// Patched elements are removed from newer archive.
    ///
    pub fn create_patch(&self, newer: &mut Self) -> Self {
        let mut patch = Self::default();

        newer.paths.retain(|k, v| {
            if let Some(base_v) = self.paths.get(k) {
                let patch_v = base_v.create_patch(v);
                if !patch_v.is_empty() {
                    patch.paths.insert(k.clone(), patch_v);
                    v.is_empty() == false
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

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.values.is_empty()
    }

    pub fn merge_onto(&mut self, other: Self) {
        // Recursively merge p
        for (k, v) in other.paths {
            self.paths.entry(k).or_default().merge_onto(v);
        }

        // Value merge is done with simple replace
        for (k, v) in other.values {
            self.values.insert(k, v);
        }
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.merge_onto(other);
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
                    let rule = rule.clone().take();

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

        deserializer.deserialize_map(PathNodeVisit {
            build: Default::default(),
        })
    }
}

impl Serialize for Archive {
    fn serialize<S>(&self, se: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = se.serialize_map(Some(self.paths.len() + self.values.len()))?;

        CATEGORY_RULE.with(|rule| {
            let rule = rule.clone().take();
            let mut key_b = CompactString::default();

            for (k, v) in &self.paths {
                rule.make_category(&k, &mut key_b);
                map.serialize_entry(&key_b, v)?;
            }

            Ok(())
        })?;

        for (k, v) in &self.values {
            debug_assert!(
                !k.starts_with("~"),
                "Tilde prefixed key '{k}' for field is not allowed!"
            );

            map.serialize_entry(k, v)?;
        }

        map.end()
    }
}

#[test]
fn test_archive_basic() {
    let src = r##"
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
    "##;

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
    assert!(sp1
        .values
        .get("value2")
        .unwrap()
        .as_object()
        .unwrap()
        .is_empty());

    let p2 = arch.paths.get("root_path_2").unwrap();
    assert!(p2.paths.is_empty());
    assert!(p2.values.len() == 4);

    let newer = r##"
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
    "##;
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

    let ref val = patch.find_path(["root_path_1", "new_path"]).unwrap().values;
    assert!(val.contains_key("valll"));
    assert!(val.get("valll") == Some(&serde_json::Value::from(4.44)));
}
