#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::*;
    use overlord_rust::{
        items::{Inventory, ItemKind},
        map::{Chest, GridPos, Provider, Requester, Structure},
        pathfinding::PathfindingAgent,
        units::{
            Unit,
            states::Available,
            tasks::{
                Action, ActionQueue, CurrentAction, CurrentTask, Reservations, Task, TaskKind,
                TaskStatus, actions_decompose_planner_system, process_current_action_system,
                update_task_completion_system,
            },
        },
    };

    // Helper pour créer un inventaire avec des items
    fn create_inventory_with(kind: ItemKind, quantity: u32) -> Inventory {
        let mut inv = Inventory::new();
        inv.add(kind, quantity);
        inv
    }

    #[test]
    fn test_get_items_task_planning() {
        // Setup app
        let mut app = App::new();

        // Add necessary resources
        app.insert_resource(Reservations::default());

        // Add systems to Update instead of FixedUpdate
        app.add_systems(Update, actions_decompose_planner_system);

        // Setup test entities
        let chest_entity = app
            .world_mut()
            .spawn((
                Structure, // Added Structure component
                Chest,
                Provider,
                create_inventory_with(ItemKind::Rock, 10),
                GridPos { x: 5, y: 5 },
            ))
            .id();

        let quantity = 8;

        let unit_entity = app
            .world_mut()
            .spawn((
                Unit {
                    name: "TestUnit".to_string(),
                },
                GridPos { x: 0, y: 0 },
                Inventory::new(),
                ActionQueue::default(),
                CurrentTask {
                    task: Some(Task::new(
                        TaskKind::GetItems {
                            kind: ItemKind::Rock,
                            quantity,
                        },
                        Vec::new(),
                    )),
                    initialized: false,
                },
                CurrentAction::default(),
                PathfindingAgent::default(),
            ))
            .id();

        // Run system
        app.update();

        // Check results
        let current_task = app.world().get::<CurrentTask>(unit_entity).unwrap();
        assert!(matches!(
            current_task.task.as_ref().unwrap().status,
            TaskStatus::Planned
        ));

        let action_queue = app.world().get::<ActionQueue>(unit_entity).unwrap();
        assert_eq!(action_queue.0.len(), 2);
        assert!(matches!(action_queue.0[0], Action::MoveTo(_)));
        assert!(
            matches!(&action_queue.0[1], Action::Take { kind: ItemKind::Rock, quantity, from: e } if *e == chest_entity)
        );

        let reservations = app.world().resource::<Reservations>();
        assert_eq!(
            reservations.owner_reserved(unit_entity, chest_entity, ItemKind::Rock),
            quantity
        );
    }

    #[test]
    fn test_take_action_execution() {
        let mut app = App::new();
        app.insert_resource(Reservations::default());
        // Changed to Update
        app.add_systems(Update, process_current_action_system);

        let chest_entity = app
            .world_mut()
            .spawn((
                Structure, // Added Structure
                Chest,
                Provider,
                create_inventory_with(ItemKind::Rock, 10),
                GridPos { x: 0, y: 0 },
            ))
            .id();

        let unit_entity = app
            .world_mut()
            .spawn((
                Unit {
                    name: "TestUnit".to_string(),
                },
                GridPos { x: 0, y: 0 },
                Inventory::new(),
                CurrentAction {
                    action: Some(Action::Take {
                        kind: ItemKind::Rock,
                        quantity: 2,
                        from: chest_entity,
                    }),
                    initialized: true,
                },
                PathfindingAgent::default(),
            ))
            .id();

        // Set up reservation
        let mut reservations = app.world_mut().resource_mut::<Reservations>();
        let chest_inv = create_inventory_with(ItemKind::Rock, 10);
        reservations.try_reserve(unit_entity, chest_entity, ItemKind::Rock, 2, &chest_inv);

        app.update();

        // Check results
        let unit_inventory = app.world().get::<Inventory>(unit_entity).unwrap();
        assert_eq!(unit_inventory.count(&ItemKind::Rock), 2);

        let chest_inventory = app.world().get::<Inventory>(chest_entity).unwrap();
        assert_eq!(chest_inventory.count(&ItemKind::Rock), 8);

        let current_action = app.world().get::<CurrentAction>(unit_entity).unwrap();
        assert!(current_action.action.is_none());

        let reservations = app.world().resource::<Reservations>();
        assert_eq!(
            reservations.owner_reserved(unit_entity, chest_entity, ItemKind::Rock),
            0
        );
    }

    #[test]
    fn test_deliver_items_task_planning() {
        let mut app = App::new();
        app.insert_resource(Reservations::default());
        // Changed to Update
        app.add_systems(Update, actions_decompose_planner_system);

        let chest_entity = app
            .world_mut()
            .spawn((
                Structure, // Added Structure
                Chest,
                Requester,
                Inventory::new(),
                GridPos { x: -5, y: -5 },
            ))
            .id();

        let unit_entity = app
            .world_mut()
            .spawn((
                Unit {
                    name: "TestUnit".to_string(),
                },
                GridPos { x: 0, y: 0 },
                create_inventory_with(ItemKind::Rock, 2),
                ActionQueue::default(),
                CurrentTask {
                    task: Some(Task::new(
                        TaskKind::DeliverItems {
                            kind: ItemKind::Rock,
                            quantity: 2,
                        },
                        Vec::new(),
                    )),
                    initialized: false,
                },
                CurrentAction::default(),
                PathfindingAgent::default(),
            ))
            .id();

        app.update();

        let current_task = app.world().get::<CurrentTask>(unit_entity).unwrap();
        assert!(matches!(
            current_task.task.as_ref().unwrap().status,
            TaskStatus::Planned
        ));

        let action_queue = app.world().get::<ActionQueue>(unit_entity).unwrap();
        assert_eq!(action_queue.0.len(), 2);
        assert!(matches!(action_queue.0[0], Action::MoveTo(_)));
        assert!(
            matches!(&action_queue.0[1], Action::Drop { kind: ItemKind::Rock, quantity: 2, to: e } if *e == chest_entity)
        );
    }

    #[test]
    fn test_drop_action_execution() {
        let mut app = App::new();
        app.insert_resource(Reservations::default());
        // Changed to Update
        app.add_systems(Update, process_current_action_system);

        let chest_entity = app
            .world_mut()
            .spawn((
                Structure, // Added Structure
                Chest,
                Requester,
                Inventory::new(),
                GridPos { x: 0, y: 0 },
            ))
            .id();

        let unit_entity = app
            .world_mut()
            .spawn((
                Unit {
                    name: "TestUnit".to_string(),
                },
                GridPos { x: 0, y: 0 },
                create_inventory_with(ItemKind::Rock, 2),
                CurrentAction {
                    action: Some(Action::Drop {
                        kind: ItemKind::Rock,
                        quantity: 2,
                        to: chest_entity,
                    }),
                    initialized: true,
                },
                PathfindingAgent::default(),
            ))
            .id();

        app.update();

        let unit_inventory = app.world().get::<Inventory>(unit_entity).unwrap();
        assert_eq!(unit_inventory.count(&ItemKind::Rock), 0);

        let chest_inventory = app.world().get::<Inventory>(chest_entity).unwrap();
        assert_eq!(chest_inventory.count(&ItemKind::Rock), 2);

        let current_action = app.world().get::<CurrentAction>(unit_entity).unwrap();
        assert!(current_action.action.is_none());
    }

    #[test]
    fn test_task_completion() {
        let mut app = App::new();
        app.insert_resource(Reservations::default());
        // Changed to Update
        app.add_systems(Update, update_task_completion_system);

        let unit_entity = app
            .world_mut()
            .spawn((
                Unit {
                    name: "TestUnit".to_string(),
                },
                Inventory::new(),
                ActionQueue::default(),
                CurrentTask {
                    task: Some(Task::new(
                        TaskKind::GetItems {
                            kind: ItemKind::Rock,
                            quantity: 2,
                        },
                        Vec::new(),
                    )),
                    initialized: false,
                },
                CurrentAction::default(),
            ))
            .id();

        // Mark task as planned and clear actions
        let mut current_task = app.world_mut().get_mut::<CurrentTask>(unit_entity).unwrap();
        current_task.task.as_mut().unwrap().status = TaskStatus::Planned;

        app.update();

        let current_task = app.world().get::<CurrentTask>(unit_entity).unwrap();
        assert!(matches!(
            current_task.task.as_ref().unwrap().status,
            TaskStatus::Completed
        ));
        assert!(app.world().get::<Available>(unit_entity).is_some());
    }
}
