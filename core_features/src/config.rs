use std::ops::Deref;
use std::string::String;
use std::{any::Any, marker::PhantomData, rc::Rc};

pub mod back {
    use super::*;

    pub struct ConfigMetadata {
        pub name: &'static str,
        pub description: &'static str,
    }

    pub struct ConfigEntityBase {
        pub meta: &'static ConfigMetadata,
        pub body: Rc<dyn Any>,
    }
}

pub struct ConfigEntity<T: Copy> {
    _p0: PhantomData<T>,
    base: back::ConfigEntityBase,
    fence: u64,
}

impl<T: 'static + Copy> ConfigEntity<T> {
    pub fn refer(&self) -> &T {
        (self.base.body.deref())
            .downcast_ref::<T>()
            .unwrap()
    }

    pub fn value(&self) -> T {
        self.refer().clone()
    }

    pub fn check_update(&mut self) -> bool {
        false
    }
}

mod tests {
    use super::*;

    #[test]
    fn try_compile() {
        static e: back::ConfigMetadata = back::ConfigMetadata {
            name: "MyNameIs",
            description: "AlphaBetaGamma",
        };

        let s = ConfigEntity::<bool> {
            _p0: PhantomData,
            base: back::ConfigEntityBase {
                meta: &e,
                body: Rc::new(false),
            },
            fence: 1,
        };

        {
            let fg = s.refer();
            assert!(!fg);
        }

        let other = s;
    }
}
