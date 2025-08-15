use bevy::prelude::*;

/// example: when units.tasks_queue.is_empty() && no currentTask
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Available;

/// example: units reached a chest and wait to interact with the chest inventory
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct WaitingToInteract {
    target: Entity,
}
