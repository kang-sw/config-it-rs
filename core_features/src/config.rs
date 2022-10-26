use serde::{Serialize};

#[derive(Serialize)]
pub struct ConfigMetadata {
    name: String,
    description: String,
    
}

pub struct ConfigEntity {}

pub mod back {}
