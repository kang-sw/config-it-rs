use std::collections::BTreeMap;

use serde::{ser::SerializeMap, Deserialize, Serialize};

type Map<T, V> = BTreeMap<T, V>;

///
/// Archived config storage.
///
#[derive(Default)]
pub struct Archive {
    pub content: Map<String, Node>,
}

#[derive(Default, Clone)]
pub struct Node {
    // Every '~' prefixed keys
    pub paths: Map<String, Node>,

    // All elements except child path nodes.
    pub values: Map<String, serde_json::Value>,
}

impl Archive {
    pub fn find_path<'s, 'a>(
        &'s self,
        path: impl IntoIterator<Item = &'a str>,
    ) -> Option<&'s Node> {
        let mut iter = path.into_iter();
        let mut paths = &self.content;
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

    pub fn merge(&mut self, other: Self) {
        for (k, v) in other.content {
            self.content.entry(k).or_default().merge(v);
        }
    }
}

impl Node {
    pub fn merge(&mut self, other: Self) {
        // Recursively merge p
        for (k, v) in other.paths {
            self.paths.entry(k).or_default().merge(v);
        }

        // Value merge is done with simple replace
        for (k, v) in other.values {
            self.values.insert(k, v);
        }
    }
}

///
/// Deserialization Logic Implementation
///
impl<'a> Deserialize<'a> for Archive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        Ok(Self {
            content: <Map<String, Node>>::deserialize(deserializer)?,
        })
    }
}

impl<'a> Deserialize<'a> for Node {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        struct PathNodeVisit {
            build: Node,
        }

        impl<'de> serde::de::Visitor<'de> for PathNodeVisit {
            type Value = Node;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("Object consist of Tilde(~) prefixed objects or ")
            }

            fn visit_map<A>(mut self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                while let Some(mut key) = map.next_key::<String>()? {
                    if !key.is_empty() && key.starts_with("~") {
                        key.remove(0); // Exclude initial tilde

                        let child: Node = map.next_value()?;
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

impl Serialize for Node {
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
fn test_load() {
    let src = r##"
        {
            "root_path_1": {
                "~subpath1": {
                    "value1": null,
                    "value2": {},
                    "~sub-subpath": {}
                },
                "~subpath2": {}
            },
            "root_path_2": {
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
    assert!(arch.content.len() == 2);

    let p1 = arch.content.get("root_path_1").unwrap();
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

    let p2 = arch.content.get("root_path_2").unwrap();
    assert!(p2.paths.is_empty());
    assert!(p2.values.len() == 4);
}
