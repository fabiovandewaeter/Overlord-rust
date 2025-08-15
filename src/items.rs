use std::collections::HashMap;

use bevy::prelude::*;

#[derive(Component)]
pub struct Item;

/// for stackable items
#[derive(Component, Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum ItemKind {
    Rock,
}

/// can't be stacked in the code but can be showed as stacked in the game UI
#[derive(Component, Clone, Copy, Debug)]
pub enum UniqueItemKind {
    IronSword,
}

/// reference to owner entity
#[derive(Component)]
pub struct InInventory {
    pub owner: Entity,
}

#[derive(Component, Default)]
pub struct Inventory {
    pub stackable_items: HashMap<ItemKind, u32>,
    // pub unique_items: Vec<Entity>,
    pub unique_items: HashMap<UniqueItemKind, Vec<Entity>>,
}

#[derive(Component)]
pub struct Durability {
    pub current: u16,
    pub max: u16,
}

// https://gemini.google.com/app/1401bd7c131ec816?hl=fr

impl Inventory {
    pub fn new() -> Self {
        Self {
            stackable_items: HashMap::new(),
            unique_items: HashMap::new(),
        }
    }

    fn add(&mut self, kind: ItemKind, quantity: u32) {
        *self.stackable_items.entry(kind).or_insert(0) += quantity;
    }

    /// remove up to qty, returns true if fully removed, false if not enough
    pub fn remove(&mut self, kind: &ItemKind, quantity: u32) -> bool {
        if let Some(current_quantity) = self.stackable_items.get_mut(kind) {
            if *current_quantity >= quantity {
                *current_quantity -= quantity;
                if *current_quantity == 0 {
                    self.stackable_items.remove(kind);
                }
                return true;
            }
        }
        false
    }

    pub fn count(&self, kind: &ItemKind) -> u32 {
        *self.stackable_items.get(kind).unwrap_or(&0)
    }

    pub fn has_item(&self, kind: ItemKind, quantity: u32) -> bool {
        self.stackable_items.get(&kind).unwrap_or(&0) >= &quantity
    }

    pub fn add_unique_item(&mut self, item_entity: Entity) {
        // self.unique_items.push(item_entity);
    }

    /// try to pop any unique item of that kind (take one)
    pub fn pop_unique_of_kind(&mut self, kind: UniqueItemKind) -> Option<Entity> {
        // if let Some(vec) = self.unique_items.get_mut(&kind) {
        //     let ent = vec.pop();
        //     if vec.is_empty() {
        //         self.unique_items.remove(&kind);
        //     }
        //     return ent;
        // }
        None
    }
}

// Système d'affichage de l'inventaire
pub fn display_inventories(inventories: Query<&Inventory>) {
    if let Ok(inventory) = inventories.single() {
        println!("=== INVENTORY ===");
        for (item_kind, quantity) in &inventory.stackable_items {
            println!("{:?}: {}", item_kind, quantity);
        }
        println!("Unique items: {}", inventory.unique_items.len());
    }
}
