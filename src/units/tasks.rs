use crate::{
    items::{CraftRecipeId, Inventory, ItemKind},
    map::{Chest, Provider, Requester, world_pos_to_tile},
    pathfinding::PathfindingAgent,
    units::{UNIT_REACH, Unit, move_and_collide_units, states::Available, update_logic},
};
use bevy::{input::common_conditions::input_pressed, prelude::*};
use std::{
    cmp::min,
    collections::{HashMap, VecDeque},
};

// TODO: see how to remove that
const BONUS_RANGE: f32 = 0.8;
const MAX_TASKS_RETRIES: u32 = 3;

pub struct TasksPlugin;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    MoveTo(Vec2),
    Craft {
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
    GetItems { kind: ItemKind, quantity: u32 }, // take from provider chest (uses reservations)
    DeliverItems { kind: ItemKind, quantity: u32 }, // go to requester chest and drop items
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Planned, // already decomposed (actions queued)
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug)]
pub struct Task {
    pub kind: TaskKind,
    pub sub_tasks: Vec<Task>,
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

#[derive(Resource, Default)]
pub struct Reservations {
    // chest -> owner -> kind -> qty
    pub reserved: HashMap<Entity, HashMap<Entity, HashMap<ItemKind, u32>>>,
}

impl Reservations {
    /// Try to reserve `qty` for `owner` on `chest`.
    /// This checks actual chest inventory (`chest_inv.count(kind)`) minus already reserved total.
    /// If enough free items exist, it records the reservation and returns true.
    pub fn try_reserve(
        &mut self,
        owner: Entity,
        chest: Entity,
        kind: ItemKind,
        qty: u32,
        chest_inv: &Inventory,
    ) -> bool {
        let total_reserved = self.total_reserved(chest, kind);
        let chest_available = chest_inv.count(&kind);

        // items not yet reserved
        let free = chest_available.saturating_sub(total_reserved);

        if free < qty {
            return false;
        }

        let owner_map = self.reserved.entry(chest).or_insert_with(HashMap::new);
        let kind_map = owner_map.entry(owner).or_insert_with(HashMap::new);
        *kind_map.entry(kind).or_insert(0) += qty;

        true
    }

    /// Release `qty` reserved by `owner` on `chest` for `kind`.
    /// If owner had less reserved than qty, it saturates to zero.
    pub fn release(&mut self, owner: Entity, chest: Entity, kind: ItemKind, qty: u32) {
        if let Some(owner_map) = self.reserved.get_mut(&chest) {
            if let Some(kind_map) = owner_map.get_mut(&owner) {
                if let Some(v) = kind_map.get_mut(&kind) {
                    *v = v.saturating_sub(qty);
                    if *v == 0 {
                        kind_map.remove(&kind);
                    }
                }
                if kind_map.is_empty() {
                    owner_map.remove(&owner);
                }
            }
            if owner_map.is_empty() {
                self.reserved.remove(&chest);
            }
        }
    }

    /// Release **all** reservations owned by `owner` on any chest.
    pub fn release_all_for_owner(&mut self, owner: Entity) {
        // collect chests to cleanup to avoid mutating while iterating
        let mut chests_to_clear: Vec<Entity> = Vec::new();
        for (chest_ent, owner_map) in self.reserved.iter_mut() {
            if owner_map.contains_key(&owner) {
                owner_map.remove(&owner);
            }
            if owner_map.is_empty() {
                chests_to_clear.push(*chest_ent);
            }
        }
        for chest in chests_to_clear {
            self.reserved.remove(&chest);
        }
    }

    /// Total reserved for a given chest and item kind (sum over all owners)
    pub fn total_reserved(&self, chest: Entity, kind: ItemKind) -> u32 {
        if let Some(owner_map) = self.reserved.get(&chest) {
            owner_map
                .values()
                .map(|kind_map| *kind_map.get(&kind).unwrap_or(&0))
                .sum()
        } else {
            0
        }
    }

    /// How much this owner specifically reserved on chest for kind
    pub fn owner_reserved(&self, owner: Entity, chest: Entity, kind: ItemKind) -> u32 {
        self.reserved
            .get(&chest)
            .and_then(|owners| owners.get(&owner))
            .and_then(|kmap| kmap.get(&kind))
            .cloned()
            .unwrap_or(0)
    }
}
// =================================================================

/// Planner: decompose Task -> Actions and attempt reservations.
/// It runs on units that have a CurrentTask (Pending) and an ActionQueue.
fn actions_decompose_planner(
    mut commands: Commands,
    mut reservations: ResMut<Reservations>,
    mut unit_query: Query<
        (
            Entity,
            &Transform,
            &mut Inventory,
            &mut ActionQueue,
            &mut CurrentTask,
        ),
        (With<Unit>, With<PathfindingAgent>),
    >,
    provider_chest_query: Query<
        (Entity, &GlobalTransform, &Inventory),
        (With<Chest>, With<Provider>, Without<Unit>),
    >,
    requester_chest_query: Query<
        (Entity, &GlobalTransform, &Inventory),
        (
            With<Chest>,
            With<Requester>,
            Without<Provider>,
            Without<Unit>,
        ),
    >,
) {
    for (unit_ent, transform, mut unit_inv, mut action_queue, mut current_task) in
        unit_query.iter_mut()
    {
        let Some(task) = &mut current_task.task else {
            continue;
        };

        // Only decompose pending tasks
        if task.status != TaskStatus::Pending {
            continue;
        }

        match task.kind {
            TaskKind::GetItems { kind, quantity } => {
                // checks if enough in unit's inventory
                let have = unit_inv.count(&kind);
                if have >= quantity {
                    task.status = TaskStatus::Completed;
                    continue;
                }

                let needed = quantity - have;
                let unit_tile_pos = world_pos_to_tile(transform.translation.xy());

                // find best chest taking into account reservations
                if let Some((chest_ent, chest_tile_pos, available)) = find_best_chest(
                    unit_tile_pos,
                    needed,
                    kind,
                    &provider_chest_query,
                    &reservations,
                ) {
                    // calculate how much we'll request to take (cap to available)
                    let take_qty = std::cmp::min(needed, available);

                    // chest inventory borrow for reservation check
                    if let Ok((_ent, _global_tf, chest_inv)) = provider_chest_query.get(chest_ent) {
                        // try to reserve
                        if reservations.try_reserve(unit_ent, chest_ent, kind, take_qty, chest_inv)
                        {
                            // Plan actions: MoveTo -> Take
                            action_queue.0.push_back(Action::MoveTo(chest_tile_pos));
                            action_queue.0.push_back(Action::Take {
                                kind,
                                quantity: take_qty,
                                from: chest_ent,
                            });

                            // mark planned so we don't plan again until this task changes
                            task.status = TaskStatus::Planned;
                            current_task.initialized = true;

                            // ensure unit is marked as busy
                            commands.entity(unit_ent).remove::<Available>();
                        } else {
                            // couldn't reserve (race or insufficient free after reservations)
                            task.status = TaskStatus::Failed;
                            println!(
                                "reservation failed for unit {:?} chest {:?}",
                                unit_ent, chest_ent
                            );
                            // cleanup any leftover reservations for this owner
                            reservations.release_all_for_owner(unit_ent);
                        }
                    } else {
                        // chest disappeared between find and reservation
                        task.status = TaskStatus::Failed;
                    }
                } else {
                    // no chest found with any stock; fallback could be craft — here we mark Failed
                    task.status = TaskStatus::Failed;
                }
            }

            TaskKind::DeliverItems { kind, quantity } => {
                // find a requester chest (here we assume single-requester scenario)
                if let Ok((requester_ent, requester_global_tf, _req_inv)) =
                    requester_chest_query.single()
                {
                    let req_pos = world_pos_to_tile(requester_global_tf.translation().xy());
                    // plan MoveTo -> Drop
                    action_queue.0.push_back(Action::MoveTo(req_pos));
                    action_queue.0.push_back(Action::Drop {
                        kind,
                        quantity,
                        to: requester_ent,
                    });

                    task.status = TaskStatus::Planned;
                    current_task.initialized = true;
                    commands.entity(unit_ent).remove::<Available>();
                } else {
                    // no requester available
                    task.status = TaskStatus::Failed;
                }
            }

            TaskKind::Action(_) => {
                // If it's a direct action wrapped as a Task, push it to ActionQueue
                if let TaskKind::Action(action) = task.kind {
                    action_queue.0.push_back(action);
                    task.status = TaskStatus::Planned;
                    commands.entity(unit_ent).remove::<Available>();
                }
            }
        }
    }
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

/// Executor: process current actions (Take/Drop/MoveTo)
pub fn process_current_action(
    mut reservations: ResMut<Reservations>,
    mut unit_query: Query<
        (
            Entity,
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
    for (unit_ent, unit_transform, mut unit_inventory, mut current_action, mut pathfinding_agent) in
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
                        current_action.action = None;
                        pathfinding_agent.target = None;
                        pathfinding_agent.path.clear();
                    }
                }

                Action::Take {
                    kind,
                    quantity,
                    from,
                } => {
                    // try to get the chest mutably
                    if let Ok((chest_ent, global_transform, mut provider_inventory)) =
                        provider_chest_query.get_mut(*from)
                    {
                        // checks if the target is at reach
                        let current_target_tile_pos =
                            world_pos_to_tile(global_transform.translation().xy());
                        let current_unit_tile_pos =
                            world_pos_to_tile(unit_transform.translation.xy());
                        let distance = current_target_tile_pos.distance(current_unit_tile_pos);
                        if distance > BONUS_RANGE + UNIT_REACH {
                            // can't reach right now -> cancel this current action (may re-try later)
                            current_action.action = None;
                            continue;
                        }

                        let available_quantity = provider_inventory.count(kind);
                        let quantity_to_take = min(*quantity, available_quantity);

                        // skip if there is no items left to take
                        if quantity_to_take == 0 {
                            // nothing to take -> release reservation for the requested amount (if any)
                            let reserved_by_owner =
                                reservations.owner_reserved(unit_ent, *from, *kind);
                            if reserved_by_owner > 0 {
                                reservations.release(unit_ent, *from, *kind, reserved_by_owner);
                            }
                            current_action.action = None;
                            continue;
                        }

                        // do the transfer (provider -> unit)
                        provider_inventory.remove(kind, quantity_to_take);
                        unit_inventory.add(*kind, quantity_to_take);

                        // reduce reservation of owner by the amount actually taken
                        let reserved_by_owner = reservations.owner_reserved(unit_ent, *from, *kind);
                        if reserved_by_owner > 0 {
                            // consume up to quantity_to_take from the reservation
                            let to_release = std::cmp::min(reserved_by_owner, quantity_to_take);
                            reservations.release(unit_ent, *from, *kind, to_release);
                            // remainder (if any) stays reserved (maybe for another scheduled Take)
                        }
                    } else {
                        // chest not found / disappeared: cancel current action
                    }
                    // whether succeeded or not, end the action (executor will continue)
                    current_action.action = None;
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
                        if quantity_to_take == 0 {
                            current_action.action = None;
                            continue;
                        }

                        unit_inventory.remove(kind, quantity_to_take);
                        requester_inventory.add(*kind, quantity_to_take);
                    }
                    current_action.action = None
                }

                Action::Craft {
                    recipe: _,
                    quantity: _,
                    with: _,
                } => {
                    // craft unimplemented in this snippet
                    current_action.action = None;
                }
            }
        }
    }
}

/// Mark tasks completed/failed and release leftovers.
/// Logic:
/// - If a unit has a Task in Planned state and both ActionQueue empty & no CurrentAction -> mark Completed and clear task + release any leftover reservations.
/// - If a Task is Failed -> release reservations & clear task (so it can be retried).
fn update_task_completion(
    mut commands: Commands,
    mut reservations: ResMut<Reservations>,
    mut unit_query: Query<
        (
            Entity,
            &mut ActionQueue,
            &mut CurrentTask,
            &mut Inventory,
            &mut CurrentAction,
        ),
        With<Unit>,
    >,
) {
    for (unit_ent, mut action_queue, mut current_task, mut inventory, mut current_action) in
        unit_query.iter_mut()
    {
        // nothing to do
        let Some(task) = &mut current_task.task else {
            continue;
        };

        match task.status {
            TaskStatus::Planned | TaskStatus::InProgress => {
                // if no pending actions and no current executing action -> task finished
                if action_queue.0.is_empty() && current_action.action.is_none() {
                    // finalize
                    task.status = TaskStatus::Completed;

                    // release any remaining reservations owned by this unit
                    reservations.release_all_for_owner(unit_ent);

                    // clear the task so we can re-request it later
                    current_task.task = None;
                    current_task.initialized = false;

                    // mark unit available
                    commands.entity(unit_ent).insert(Available);
                }
            }
            TaskStatus::Failed => {
                // free reservations and clear task to allow retrial
                reservations.release_all_for_owner(unit_ent);
                current_task.task = None;
                current_task.initialized = false;
                commands.entity(unit_ent).insert(Available);
            }
            TaskStatus::Pending | TaskStatus::Completed => {
                // nothing special
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
    reservations: &Reservations,
) -> Option<(Entity, Vec2, u32)> {
    let mut best_with_enough: Option<(Entity, Vec2, u32, f32)> = None; // (entity, tile, qty, distance)
    let mut best_any: Option<(Entity, Vec2, u32, f32)> = None; // nearest with at least 1

    for (chest_ent, chest_global_transform, chest_inv) in chest_query.iter() {
        let chest_tile = world_pos_to_tile(chest_global_transform.translation().xy());
        let dist = unit_tile_pos.distance(chest_tile);
        let real_quantity = chest_inv.count(&desired_item_kind);
        let already_reserved = reservations.total_reserved(chest_ent, desired_item_kind);

        let available = real_quantity.saturating_sub(already_reserved);

        if available == 0 {
            continue;
        }

        if available >= desired_quantity {
            match &best_with_enough {
                None => best_with_enough = Some((chest_ent, chest_tile, available, dist)),
                Some((_, _, _, best_dist)) if dist < *best_dist => {
                    best_with_enough = Some((chest_ent, chest_tile, available, dist))
                }
                _ => {}
            }
        } else {
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

/// Test helper: assign a GetItems task when pressing E (safe: only assign when no current task or previous task completed/failed)
fn test_find_2_rocks(
    mut unit_query: Query<
        (&Transform, &mut ActionQueue, &mut CurrentTask),
        (With<Unit>, With<PathfindingAgent>),
    >,
) {
    let mut counter = 0;
    for (_unit_transform, _unit_action_queue, mut unit_current_task) in unit_query.iter_mut() {
        if counter > 0 {
            return;
        }
        // Only assign if there's no task or last task finished/failed
        let assign = match &unit_current_task.task {
            None => true,
            Some(t) => matches!(t.status, TaskStatus::Completed | TaskStatus::Failed),
        };
        if assign {
            let find_2_rocks = Task::new(
                TaskKind::GetItems {
                    kind: ItemKind::Rock,
                    quantity: 2,
                },
                Vec::new(),
            );
            unit_current_task.task = Some(find_2_rocks);
            unit_current_task.initialized = false;
        }
        counter += 1;
    }
}

/// Test helper: assign a DeliverItems task that goes to the requester chest and drops 2 rocks
fn test_deliver_2_rocks(
    mut unit_query: Query<
        (&Transform, &mut ActionQueue, &mut CurrentTask),
        (With<Unit>, With<PathfindingAgent>),
    >,
) {
    let mut counter = 0;
    for (_unit_transform, _unit_action_queue, mut unit_current_task) in unit_query.iter_mut() {
        if counter > 0 {
            return;
        }
        // assign only if idle or previous task finished/failed
        let assign = match &unit_current_task.task {
            None => true,
            Some(t) => matches!(t.status, TaskStatus::Completed | TaskStatus::Failed),
        };
        if assign {
            let deliver_2_rocks = Task::new(
                TaskKind::DeliverItems {
                    kind: ItemKind::Rock,
                    quantity: 2,
                },
                Vec::new(),
            );
            unit_current_task.task = Some(deliver_2_rocks);
            unit_current_task.initialized = false;
        }
        counter += 1;
    }
}

pub fn display_reservations(reservations: Res<Reservations>, unit_query: Query<&CurrentAction>) {
    println!("reservations: {:?}", reservations.reserved);
    for current_action in unit_query.iter() {
        if let Some(action) = &current_action.action {
            println!("current_action: {:?}", action);
        }
    }
}

impl Plugin for TasksPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.insert_resource(Reservations::default()).add_systems(
            FixedUpdate,
            (
                actions_decompose_planner.before(process_current_action),
                process_current_action.before(move_and_collide_units),
                update_task_completion.after(process_current_action),
                assign_next_action_or_set_available.after(update_logic),
                // tests:
                test_find_2_rocks.run_if(input_pressed(KeyCode::KeyE)),
                // you can bind another key for deliver test, e.g. R
                test_deliver_2_rocks.run_if(input_pressed(KeyCode::KeyR)),
            ),
        );
    }
}
