use bevy::prelude::*;

// TODO: use that to find all idle units and assign tasks to them
#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct Idle;
