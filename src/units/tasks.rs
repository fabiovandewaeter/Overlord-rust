use crate::{
    items::{CraftRecipeId, Inventory, ItemKind},
    map::{Chest, Provider, Requester, world_pos_to_tile},
    pathfinding::PathfindingAgent,
    units::{UNIT_REACH, Unit, move_and_collide_units, states::Available, update_logic},
};
use bevy::{input::common_conditions::input_pressed, prelude::*};
use std::{cmp::min, collections::VecDeque};

// TODO: see how to remove that
const BONUS_RANGE: f32 = 0.8;
const MAX_TASKS_RETRIES: u32 = 3;

pub struct TasksPlugin;

https://chatgpt.com/c/68a3786f-43c8-832b-b239-f2b7103a13b5
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    MoveTo(Vec2),
    Craft {
        // kind: ItemKind,
        recipe: CraftRecipeId,
        quantity: u32, // while enough items
        with: Entity,  // crafting machine
    },
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
pub struct ActionQueue(pub VecDeque<Action>);

impl From<Vec<Action>> for ActionQueue {
    fn from(v: Vec<Action>) -> Self {
        Self(v.into())
    }
}

#[derive(Component, Default)]
pub struct CurrentAction {
    pub action: Option<Action>,
    pub initialized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskKind {
    Action(Action),
    FindItems { kind: ItemKind, quantity: u32 }, // check in inventory, then in Chest (do an Action::Take), otherwise do an Action::Craft
}
// FindItem {// check in inventory, then in Chest (do an Action::Take), otherwise do an Action::Craft
// },
// Craft {// find items (with an Action::FindItem then craft)
// },

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Planned, // already decomposed
    InProgress,
    Completed,
    Failed,
}

// https://chatgpt.com/c/68a35fb0-31ac-8333-a7f9-e963b3ea99b3
#[derive(Debug)]
pub struct Task {
    pub kind: TaskKind,
    pub sub_tasks: Vec<Task>, // example: Task::FindItem(Iron) that will be decompose as Task::FindItem(IronNuggets), then Task::Craft(Iron) from Action::MoveTo(Furnace) + Action::Craft(Iron)
    pub status: TaskStatus,
}

impl Task {
    pub fn new(kind: TaskKind, sub_tasks: Vec<Task>) -> Self {
        Self {
            kind,
            sub_tasks,
            status: TaskStatus::Pending,
        }
    }
}

#[derive(Component, Default)]
pub struct CurrentTask {
    pub task: Option<Task>,
    pub initialized: bool,
}
// =================================================================

/// example: for a Task::CraftTable, it creates some Task::GetResources that can creates Action::MoveTo and Action::Take from a Chest
fn actions_decompose_planner() {
    // Action::FindItem { kind, quantity } => {
    //     if unit_inventory.count(kind) >= *quantity {
    //         continue;
    //     }
    //     // else, tries to find a Chest with enough items
    //     let unit_tile_pos = world_pos_to_tile(unit_transform.translation.xy());

    //     if let Some((chest_entity, chest_tile_pos, available_quantity)) =
    //         find_best_chest(unit_tile_pos, *quantity, *kind, &provider_chest_query)
    //     {
    //     } else {
    //         // else, craft them
    //     }
    // }
}

pub fn reset_actions(
    action_queue: &mut ActionQueue,
    current_action: &mut CurrentAction,
    pathfinding_agent: &mut PathfindingAgent,
) {
    action_queue.0.clear();
    if let Some(action) = &current_action.action {
        match action {
            Action::MoveTo(_) => {
                // resets pathfinding_agent
                pathfinding_agent.target = None;
                pathfinding_agent.path.clear();
            }
            _ => {}
        };
    }
    current_action.action = None;
    current_action.initialized = false;
}

/// pops the front of the ActionQueue to get the next CurrentAction ; add Available component if unit has no more actions to do
pub fn assign_next_action_or_set_available(
    mut commands: Commands,
    mut unit_query: Query<
        (Entity, &mut ActionQueue, &mut CurrentAction),
        (With<Unit>, Without<Available>),
    >,
) {
    for (entity, mut action_queue, mut current_action) in unit_query.iter_mut() {
        if current_action.action.is_none() {
            if let Some(next_action) = action_queue.0.pop_front() {
                current_action.action = Some(next_action);
                current_action.initialized = false;
            } else {
                commands.entity(entity).insert(Available);
            }
        }
    }
}

/// example : take an item from a chest if it's the current action
pub fn process_current_action(
    mut unit_query: Query<
        (
            &Transform,
            &mut Inventory,
            &mut CurrentAction,
            &mut PathfindingAgent,
        ),
        With<Unit>,
    >,
    mut provider_chest_query: Query<
        (Entity, &GlobalTransform, &mut Inventory),
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
    for (unit_transform, mut unit_inventory, mut current_action, mut pathfinding_agent) in
        unit_query.iter_mut()
    {
        // skip if there is no current action
        if current_action.action.is_none() {
            continue;
        }
        // initialize if not initialized yet
        if !current_action.initialized {
            match &current_action.action {
                Some(Action::MoveTo(target_pos)) => {
                    pathfinding_agent.target = Some(*target_pos);
                    pathfinding_agent.path.clear();
                }
                _ => {}
            }
            current_action.initialized = true;
        }

        if let Some(action) = &mut current_action.action {
            match action {
                Action::MoveTo(target_pos) => {
                    let current_unit_tile_pos = world_pos_to_tile(unit_transform.translation.xy());
                    let distance = current_unit_tile_pos.distance(*target_pos);

                    // Si on est assez proche de la destination, considérer la tâche terminée
                    if pathfinding_agent.path.is_empty() && distance <= BONUS_RANGE + UNIT_REACH {
                        // Ajustez cette valeur selon vos besoins
                        current_action.action = None;
                        // reset pathfinding_agent
                        pathfinding_agent.target = None;
                        pathfinding_agent.path.clear();
                    }
                }
                Action::Take {
                    kind,
                    quantity,
                    from,
                } => {
                    if let Ok((_, global_transform, mut provider_inventory)) =
                        provider_chest_query.get_mut(*from)
                    {
                        // checks if the target is at reach
                        let current_target_tile_pos =
                            world_pos_to_tile(global_transform.translation().xy());
                        let current_unit_tile_pos =
                            world_pos_to_tile(unit_transform.translation.xy());
                        let distance = current_target_tile_pos.distance(current_unit_tile_pos);
                        if distance > BONUS_RANGE + UNIT_REACH {
                            current_action.action = None;
                            continue;
                        }

                        let available_quantity = provider_inventory.count(kind);
                        let quantity_to_take = min(*quantity, available_quantity);

                        // skip if there is no items left to take
                        if quantity_to_take <= 0 {
                            current_action.action = None;
                            continue;
                        }

                        provider_inventory.remove(kind, quantity_to_take);
                        unit_inventory.add(*kind, quantity_to_take);
                    }
                    current_action.action = None // whether it succeeds or not
                }
                Action::Drop { kind, quantity, to } => {
                    if let Ok((global_transform, mut requester_inventory)) =
                        requester_chest_query.get_mut(*to)
                    {
                        // checks if the target is at reach
                        let current_target_tile_pos =
                            world_pos_to_tile(global_transform.translation().xy());
                        let current_unit_tile_pos =
                            world_pos_to_tile(unit_transform.translation.xy());
                        let distance = current_target_tile_pos.distance(current_unit_tile_pos);
                        if distance > BONUS_RANGE + UNIT_REACH {
                            current_action.action = None;
                            continue;
                        }

                        let available_quantity = unit_inventory.count(kind);
                        let quantity_to_take = min(*quantity, available_quantity);

                        // skip if there is no items left to take
                        if quantity_to_take <= 0 {
                            current_action.action = None;
                            continue;
                        }

                        unit_inventory.remove(kind, quantity_to_take);
                        requester_inventory.add(*kind, quantity_to_take);
                    }
                    current_action.action = None // whether it succeeds or not
                }
                Action::Craft {
                    recipe,
                    quantity,
                    with,
                } => todo!(),
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

fn test_add_move_to_then_take_rocks_from_chest_action(
    mut commands: Commands,
    mut unit_query: Query<
        (Entity, &Transform, &mut ActionQueue),
        (With<Unit>, With<PathfindingAgent>),
    >,
    provider_chest_query: Query<
        (Entity, &GlobalTransform, &Inventory),
        (With<Chest>, With<Provider>, Without<Unit>),
    >,
    requester_chest_query: Query<
        (Entity, &GlobalTransform),
        (With<Chest>, With<Inventory>, With<Requester>, Without<Unit>),
    >,
) {
    const DESIRED_QUANTITY: u32 = 10;
    const DESIRED_KIND: ItemKind = ItemKind::Rock;
    for (unit_entity, unit_transform, mut action_queue) in unit_query.iter_mut() {
        let unit_tile_pos = world_pos_to_tile(unit_transform.translation.xy());

        if let Some((chest_entity, chest_tile_pos, available_quantity)) = find_best_chest(
            unit_tile_pos,
            DESIRED_QUANTITY,
            DESIRED_KIND,
            &provider_chest_query,
        ) {
            let take_quantity = std::cmp::min(DESIRED_QUANTITY, available_quantity);

            if let Ok((requester_chest_entity, requester_chest_global_transform)) =
                requester_chest_query.single()
            {
                let requester_chest_pos =
                    world_pos_to_tile(requester_chest_global_transform.translation().xy());
                // drop item in requester chest
                action_queue.0.push_front(Action::Drop {
                    kind: DESIRED_KIND,
                    quantity: 10000000,
                    to: requester_chest_entity,
                });
                action_queue
                    .0
                    .push_front(Action::MoveTo(requester_chest_pos));
            }

            // take
            action_queue.0.push_front(Action::Take {
                kind: DESIRED_KIND,
                quantity: take_quantity,
                from: chest_entity,
            });
            action_queue.0.push_front(Action::MoveTo(chest_tile_pos));

            commands.entity(unit_entity).remove::<Available>();
        } else {
            println!("no chest found");
        }
    }
}

fn test_find_2_rocks(
    mut unit_query: Query<
        (Entity, &Transform, &ActionQueue, &mut CurrentTask),
        (With<Unit>, With<PathfindingAgent>),
    >,
) {
    let mut counter = 0;
    for (unit_entity, unit_transform, unit_action_queue, mut unit_current_task) in
        unit_query.iter_mut()
    {
        // try with only one unit
        if counter >= 0 {
            return;
        }
        let find_2_rocks = Task::new(
            TaskKind::FindItems {
                kind: ItemKind::Rock,
                quantity: 2,
            },
            Vec::new(),
        );
        unit_current_task.task = Some(find_2_rocks);

        counter += 1;
    }
}

impl Plugin for TasksPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(
            FixedUpdate,
            (
                actions_decompose_planner.before(process_current_action),
                assign_next_action_or_set_available.after(update_logic),
                process_current_action.before(move_and_collide_units),
                test_add_move_to_then_take_rocks_from_chest_action
                    .run_if(input_pressed(KeyCode::KeyE)),
            ),
        );
    }
}
