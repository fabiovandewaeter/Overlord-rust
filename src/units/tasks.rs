use crate::{
    items::{Inventory, ItemKind},
    map::{Chest, TILE_SIZE, world_pos_to_tile},
    pathfinding::PathfindingAgent,
    units::{UNIT_REACH, Unit, move_and_collide_units, states::Available, update_logic},
};
use bevy::{input::common_conditions::input_pressed, prelude::*};
use std::{cmp::min, collections::VecDeque};

pub struct TasksPlugin;

#[derive(Debug)]
pub enum Task {
    MoveTo(Vec2),
    Take {
        kind: ItemKind,
        quantity: u32,
        from: Entity,
    },
}

#[derive(Component, Default, Debug)]
pub struct TaskQueue(pub VecDeque<Task>);

impl From<Vec<Task>> for TaskQueue {
    fn from(v: Vec<Task>) -> Self {
        Self(v.into())
    }
}

#[derive(Component, Default)]
pub struct CurrentTask(pub Option<Task>);

/// pops the front of the TaskQueue to get the next CurrentTask ; add Available component if unit has no more tasks to do
pub fn assign_next_task_or_set_available(
    mut commands: Commands,
    mut unit_query: Query<
        (Entity, &mut TaskQueue, &mut CurrentTask),
        (With<Unit>, Without<Available>),
    >,
) {
    for (entity, mut task_queue, mut current_task) in unit_query.iter_mut() {
        if current_task.0.is_none() {
            if let Some(next_task) = task_queue.0.pop_front() {
                current_task.0 = Some(next_task);
            } else {
                commands.entity(entity).insert(Available);
            }
        }
    }
}

/// example : take an item from a chest if it's the current task
pub fn process_current_task(
    mut unit_query: Query<(&Transform, &mut Inventory, &mut CurrentTask), With<Unit>>,
    mut chest_query: Query<(&Transform, &mut Inventory), (With<Chest>, Without<Unit>)>,
) {
    for (unit_transform, mut unit_inventory, mut current_task) in unit_query.iter_mut() {
        if let Some(task) = &mut current_task.0 {
            match task {
                Task::MoveTo(vec2) => {
                    println!("test1");
                }
                Task::Take {
                    kind,
                    quantity,
                    from,
                } => {
                    println!("test2");
                    if let Ok((transform, mut source_inventory)) = chest_query.get_mut(*from) {
                        // checks if the target is at reach
                        let current_target_tile_pos = world_pos_to_tile(transform.translation.xy());
                        let current_unit_tile_pos =
                            world_pos_to_tile(unit_transform.translation.xy());
                        let distance = current_target_tile_pos.distance(current_unit_tile_pos);
                        if distance > UNIT_REACH {
                            current_task.0 = None;
                            continue;
                        }

                        let available_quantity = source_inventory.count(kind);
                        let quantity_to_take = min(*quantity, available_quantity);

                        // skip if there is no items left to take
                        if quantity_to_take <= 0 {
                            current_task.0 = None;
                            continue;
                        }

                        source_inventory.remove(kind, quantity_to_take);
                        unit_inventory.add(*kind, quantity_to_take);
                    } else {
                    }
                    current_task.0 = None // whether it succeeds or not
                }
                _ => {}
            }
        }
    }
}

fn find_best_chest(
    unit_tile_pos: Vec2,
    desired_quantity: u32,
    desired_item_kind: ItemKind,
    chest_query: &Query<(Entity, &GlobalTransform, &Inventory), (With<Chest>, Without<Unit>)>,
) -> Option<(Entity, Vec2, u32)> {
    let mut best_with_enough: Option<(Entity, Vec2, u32, f32)> = None; // (entity, tile, qty, distance)
    let mut best_any: Option<(Entity, Vec2, u32, f32)> = None; // nearest with at least 1

    for (chest_ent, chest_global_transform, chest_inv) in chest_query.iter() {
        let chest_tile = world_pos_to_tile(chest_global_transform.translation().xy());
        let dist = unit_tile_pos.distance(chest_tile);
        let available = chest_inv.count(&desired_item_kind);

        if available == 0 {
            continue;
        }

        if available >= desired_quantity {
            // candidate qui a assez
            match &best_with_enough {
                None => best_with_enough = Some((chest_ent, chest_tile, available, dist)),
                Some((_, _, _, best_dist)) if dist < *best_dist => {
                    best_with_enough = Some((chest_ent, chest_tile, available, dist))
                }
                _ => {}
            }
        } else {
            // candidate avec >=1 mais < desired
            match &best_any {
                None => best_any = Some((chest_ent, chest_tile, available, dist)),
                Some((_, _, _, best_dist)) if dist < *best_dist => {
                    best_any = Some((chest_ent, chest_tile, available, dist))
                }
                _ => {}
            }
        }
    }

    if let Some((e, pos, qty, _)) = best_with_enough {
        Some((e, pos, qty))
    } else if let Some((e, pos, qty, _)) = best_any {
        Some((e, pos, qty))
    } else {
        None
    }
}

fn add_move_to_then_take_rocks_from_chest_task(
    mut commands: Commands,
    mut unit_query: Query<(Entity, &Transform, &mut PathfindingAgent, &mut TaskQueue), With<Unit>>,
    chest_query: Query<(Entity, &GlobalTransform, &Inventory), (With<Chest>, Without<Unit>)>,
) {
    const DESIRED_QUANTITY: u32 = 10;
    const DESIRED_KIND: ItemKind = ItemKind::Rock;
    for (unit_entity, unit_transform, mut pathfinding_agent, mut task_queue) in
        unit_query.iter_mut()
    {
        let unit_tile_pos = world_pos_to_tile(unit_transform.translation.xy());

        if let Some((chest_entity, chest_tile_pos, available_quantity)) =
            find_best_chest(unit_tile_pos, DESIRED_QUANTITY, DESIRED_KIND, &chest_query)
        {
            let take_quantity = std::cmp::min(DESIRED_QUANTITY, available_quantity);
            task_queue.0.push_front(Task::Take {
                kind: DESIRED_KIND,
                quantity: take_quantity,
                from: chest_entity,
            });
            task_queue.0.push_front(Task::MoveTo(chest_tile_pos));

            // reset pathfingin_agent
            // pathfinding_agent.target = Some(chest_tile_pos);
            let test_tile_pos = world_pos_to_tile(Vec2::new(5.0 * TILE_SIZE.x, 4.0 * TILE_SIZE.y));
            println!("chest_tile_pos: {:?} {:?}", chest_tile_pos, test_tile_pos);
            // pathfinding_agent.target = Some(test_tile_pos);
            pathfinding_agent.target = Some(chest_tile_pos);
            pathfinding_agent.path.clear();

            commands.entity(unit_entity).remove::<Available>();
        } else {
            println!("no chest found");
        }
    }
}

impl Plugin for TasksPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(
            FixedUpdate,
            (
                assign_next_task_or_set_available.after(update_logic),
                process_current_task.before(move_and_collide_units),
                add_move_to_then_take_rocks_from_chest_task.run_if(input_pressed(KeyCode::KeyE)),
            ),
        );
    }
}
