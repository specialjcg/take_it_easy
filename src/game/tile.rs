use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Copy,Hash,Eq,Serialize,Deserialize)]
pub(crate) struct Tile(pub i32, pub i32, pub i32);