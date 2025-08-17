use crate::{
    items::{Inventory, ItemKind},
    map::{Chest, Provider, Requester, TILE_SIZE, world_pos_to_tile},
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
    Drop {
        kind: ItemKind,
        quantity: u32,
        to: Entity,
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
pub struct CurrentTask {
    pub task: Option<Task>,
    pub initialized: bool,
}

/// pops the front of the TaskQueue to get the next CurrentTask ; add Available component if unit has no more tasks to do
pub fn assign_next_task_or_set_available(
    mut commands: Commands,
    mut unit_query: Query<
        (Entity, &mut TaskQueue, &mut CurrentTask),
        (With<Unit>, Without<Available>),
    >,
) {
    for (entity, mut task_queue, mut current_task) in unit_query.iter_mut() {
        if current_task.task.is_none() {
            if let Some(next_task) = task_queue.0.pop_front() {
                current_task.task = Some(next_task);
                current_task.initialized = false;
            } else {
                commands.entity(entity).insert(Available);
            }
        }
    }
}

/// example : take an item from a chest if it's the current task
pub fn process_current_task(
    mut unit_query: Query<
        (
            &Transform,
            &mut Inventory,
            &mut CurrentTask,
            &mut PathfindingAgent,
        ),
        With<Unit>,
    >,
    mut provider_chest_query: Query<
        (&GlobalTransform, &mut Inventory),
        (With<Chest>, With<Provider>, Without<Unit>),
    >,
    mut requester_chest_query: Query<
        (&GlobalTransform, &mut Inventory),
        (
            With<Chest>,
            With<Requester>,
            Without<Provider>,
            Without<Unit>,
        ),
    >,
) {
    for (unit_transform, mut unit_inventory, mut current_task, mut pathfinding_agent) in
        unit_query.iter_mut()
    {
        // skip if there is no current task
        if current_task.task.is_none() {
            continue;
        }
        // initialize if not initialized yet
        if !current_task.initialized {
            match &current_task.task {
                Some(Task::MoveTo(target_pos)) => {
                    pathfinding_agent.target = Some(*target_pos);
                    pathfinding_agent.path.clear();
                }
                _ => {}
            }
            current_task.initialized = true;
        }

        if let Some(task) = &mut current_task.task {
            match task {
                Task::MoveTo(target_pos) => {
                    let current_unit_tile_pos = world_pos_to_tile(unit_transform.translation.xy());
                    let distance = current_unit_tile_pos.distance(*target_pos);

                    // Si on est assez proche de la destination, considérer la tâche terminée
                    // if distance <= 0.8 + UNIT_REACH {
                    if pathfinding_agent.path.is_empty() && distance <= 0.8 + UNIT_REACH {
                        // Ajustez cette valeur selon vos besoins
                        current_task.task = None;
                        // Optionnel : arrêter le pathfinding
                        pathfinding_agent.target = None;
                        pathfinding_agent.path.clear();
                    }
                }
                Task::Take {
                    kind,
                    quantity,
                    from,
                } => {
                    if let Ok((global_transform, mut provider_inventory)) =
                        provider_chest_query.get_mut(*from)
                    {
                        // checks if the target is at reach
                        let current_target_tile_pos =
                            world_pos_to_tile(global_transform.translation().xy());
                        let current_unit_tile_pos =
                            world_pos_to_tile(unit_transform.translation.xy());
                        let distance = current_target_tile_pos.distance(current_unit_tile_pos);
                        if distance > 0.8 + UNIT_REACH {
                            current_task.task = None;
                            continue;
                        }

                        let available_quantity = provider_inventory.count(kind);
                        let quantity_to_take = min(*quantity, available_quantity);

                        // skip if there is no items left to take
                        if quantity_to_take <= 0 {
                            current_task.task = None;
                            continue;
                        }

                        provider_inventory.remove(kind, quantity_to_take);
                        unit_inventory.add(*kind, quantity_to_take);
                    }
                    current_task.task = None // whether it succeeds or not
                }
                Task::Drop { kind, quantity, to } => {
                    if let Ok((global_transform, mut requester_inventory)) =
                        requester_chest_query.get_mut(*to)
                    {
                        // checks if the target is at reach
                        let current_target_tile_pos =
                            world_pos_to_tile(global_transform.translation().xy());
                        let current_unit_tile_pos =
                            world_pos_to_tile(unit_transform.translation.xy());
                        let distance = current_target_tile_pos.distance(current_unit_tile_pos);
                        if distance > 0.8 + UNIT_REACH {
                            current_task.task = None;
                            continue;
                        }

                        let available_quantity = unit_inventory.count(kind);
                        let quantity_to_take = min(*quantity, available_quantity);

                        // skip if there is no items left to take
                        if quantity_to_take <= 0 {
                            current_task.task = None;
                            continue;
                        }

                        unit_inventory.remove(kind, quantity_to_take);
                        requester_inventory.add(*kind, quantity_to_take);
                    }
                    current_task.task = None // whether it succeeds or not
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
    chest_query: &Query<
        (Entity, &GlobalTransform, &Inventory),
        (With<Chest>, With<Provider>, Without<Unit>),
    >,
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
    provider_chest_query: Query<
        (Entity, &GlobalTransform, &Inventory),
        (With<Chest>, With<Provider>, Without<Unit>),
    >,
    requester_chest_query: Query<
        (Entity, &GlobalTransform, &Inventory),
        (With<Chest>, With<Requester>, Without<Unit>),
    >,
) {
    const DESIRED_QUANTITY: u32 = 10;
    const DESIRED_KIND: ItemKind = ItemKind::Rock;
    for (unit_entity, unit_transform, mut pathfinding_agent, mut task_queue) in
        unit_query.iter_mut()
    {
        let unit_tile_pos = world_pos_to_tile(unit_transform.translation.xy());

        if let Some((chest_entity, chest_tile_pos, available_quantity)) = find_best_chest(
            unit_tile_pos,
            DESIRED_QUANTITY,
            DESIRED_KIND,
            &provider_chest_query,
        ) {
            let take_quantity = std::cmp::min(DESIRED_QUANTITY, available_quantity);

            if let Ok((
                requester_chest_entity,
                requester_chest_global_transform,
                requester_chest_inventory,
            )) = requester_chest_query.single()
            {
                // let requester_chest_pos =
                //     world_pos_to_tile(requester_chest_global_transform.translation().xy());
                let requester_chest_pos = Vec2::new(-5.0, 5.0);
                // drop
                task_queue.0.push_front(Task::Drop {
                    kind: DESIRED_KIND,
                    quantity: 10000000,
                    to: requester_chest_entity,
                });
                task_queue.0.push_front(Task::MoveTo(requester_chest_pos));
            }

            // take
            task_queue.0.push_front(Task::Take {
                kind: DESIRED_KIND,
                quantity: take_quantity,
                from: chest_entity,
            });
            task_queue.0.push_front(Task::MoveTo(chest_tile_pos));

            // reset pathfingin_agent
            // pathfinding_agent.target = Some(chest_tile_pos);
            // pathfinding_agent.path.clear();

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
