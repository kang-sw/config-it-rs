use std::collections::BTreeMap;

use compact_str::CompactString;
use serde::{ser::SerializeMap, Deserialize, Serialize};
use smallvec::SmallVec;

type Map<T, V> = BTreeMap<T, V>;

#[derive(Default, Clone, Debug)]
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
    pub fn patch(base: &Self, newer: &Self) -> Self {
        let mut patch = Self::default();

        for (k, v) in &newer.paths {
            if let Some(base_v) = base.paths.get(k) {
                let patch_v = Self::patch(base_v, v);
                if !patch_v.is_empty() {
                    patch.paths.insert(k.clone(), patch_v);
                }
            } else {
                patch.paths.insert(k.clone(), v.clone());
            }
        }

        for (k, v) in &newer.values {
            if let Some(base_v) = base.values.get(k) {
                if *base_v != *v {
                    patch.values.insert(k.clone(), v.clone());
                }
            } else {
                patch.values.insert(k.clone(), v.clone());
            }
        }

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
                while let Some(mut key) = map.next_key::<CompactString>()? {
                    if !key.is_empty() && key.starts_with("~") {
                        key.remove(0); // Exclude initial tilde

                        let child: Archive = map.next_value()?;
                        self.build.paths.insert(key, child);
                    } else {
                        let value: serde_json::Value = map.next_value()?;
                        self.build.values.insert(key, value);
                    }
                }

                Ok(self.build)
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
        let mut key_b = String::with_capacity(10);

        for (k, v) in &self.paths {
            key_b.push('~');
            key_b.push_str(&k);
            map.serialize_entry(&key_b, v)?;
            key_b.clear();
        }

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
    let patch = Archive::patch(&arch, &newer);

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
