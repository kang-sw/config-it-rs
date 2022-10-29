pub mod front;
pub mod registry;
pub mod storage;
pub mod entity;

mod __all {
    pub use super::front::*;
    pub use super::registry::*;
    pub use super::storage::*;
    pub use super::entity::*;

    pub type JsonObject = serde_json::Value;
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
