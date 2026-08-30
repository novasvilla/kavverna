use crate::item::Item;
use serde::{Deserialize, Serialize};

/// One drop gesture. Several things let go together arrive as one pile; a pile of one
/// renders as a plain item. Dragging the pile drags everything in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pile {
    pub id: u64,
    pub items: Vec<Item>,
}
