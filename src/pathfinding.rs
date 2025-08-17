use crate::map::{
    self, StructureManager, get_neighbors, is_tile_passable, tile_pos_to_rounded_tile,
    world_pos_to_tile,
};
use crate::units::MovementSpeed;
use crate::units::tasks::CurrentTask;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};

pub struct PathfindingPlugin;

#[derive(Clone)]
struct PathNode {
    pos: IVec2,
    g_cost: f32,
    h_cost: f32,
    parent: Option<IVec2>,
}

impl PathNode {
    fn f_cost(&self) -> f32 {
        self.g_cost + self.h_cost
    }
}
impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f_cost()
            .partial_cmp(&self.f_cost())
            .unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for PathNode {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos
    }
}
impl Eq for PathNode {}

// ========== FONCTIONS UTILITAIRES ==========

/// Convertit une position de grille A* en position logique (centre de la tuile).
pub fn grid_to_tile_pos(grid_pos: IVec2) -> Vec2 {
    Vec2::new(grid_pos.x as f32 + 0.5, grid_pos.y as f32 + 0.5)
}

// ========== PATHFINDING UTILISANT DIRECTEMENT VOTRE TILEMAP ==========

#[derive(Component)]
pub struct PathfindingAgent {
    pub target: Option<Vec2>,
    pub path: VecDeque<Vec2>,
    pub path_tolerance: f32,
}

impl Default for PathfindingAgent {
    fn default() -> Self {
        PathfindingAgent {
            target: None,
            path: VecDeque::new(),
            path_tolerance: 0.1, // 10% de la taille d'une tile
        }
    }
}

/// checks if there is a structure at rounded_tile_pos

fn is_diagonal(from: IVec2, to: IVec2) -> bool {
    (from.x - to.x).abs() == 1 && (from.y - to.y).abs() == 1
}

fn heuristic(a: IVec2, b: IVec2) -> f32 {
    let dx = (a.x - b.x) as f32;
    let dy = (a.y - b.y) as f32;
    (dx * dx + dy * dy).sqrt()
}

fn reconstruct_path(all_nodes: &HashMap<IVec2, PathNode>, mut current: IVec2) -> VecDeque<Vec2> {
    let mut path = VecDeque::new();
    while let Some(node) = all_nodes.get(&current) {
        path.push_front(grid_to_tile_pos(node.pos));
        if let Some(parent) = node.parent {
            current = parent;
        } else {
            break;
        }
    }
    path
}

// Dans votre fonction find_path, remplacez cette partie :

// Dans votre fonction find_path, remplacez cette partie :

fn find_path(
    start_pos: Vec2,
    end_pos: Vec2,
    structure_manager: &Res<StructureManager>,
) -> Option<VecDeque<Vec2>> {
    let start_grid = tile_pos_to_rounded_tile(start_pos);
    let end_grid = tile_pos_to_rounded_tile(end_pos);

    // ❌ ANCIEN CODE - causait le problème
    // if !is_tile_passable(end_grid, structure_manager) {
    //     return None;
    // }

    // ✅ NOUVEAU CODE - trouve la destination la plus proche si la cible n'est pas passable
    let actual_end_grid = if !is_tile_passable(end_grid, structure_manager) {
        // Si la destination n'est pas passable, on cherche la case passable la plus proche
        // en tenant compte de la direction d'approche depuis start_grid
        find_nearest_passable_tile(end_grid, start_grid, structure_manager).unwrap_or(start_grid)
    } else {
        end_grid
    };

    // -------------------------
    // Configuration de la limite (reste identique)
    // -------------------------
    const BASE_LIMIT: usize = 500;
    const PER_TILE_LIMIT: usize = 40;
    const MAX_LIMIT: usize = 20_000;

    let dist_tiles = heuristic(start_grid, actual_end_grid);
    let per_tile_extra = ((dist_tiles).round() as isize).max(0) as usize;
    let mut max_expansions = BASE_LIMIT + per_tile_extra * PER_TILE_LIMIT;
    if max_expansions > MAX_LIMIT {
        max_expansions = MAX_LIMIT;
    }

    let return_partial_on_limit = true;

    // -------------------------
    // A* habituel (reste identique, mais utilise actual_end_grid)
    // -------------------------
    let mut open_set = BinaryHeap::new();
    let mut all_nodes: HashMap<IVec2, PathNode> = HashMap::new();

    let start_node = PathNode {
        pos: start_grid,
        g_cost: 0.0,
        h_cost: heuristic(start_grid, actual_end_grid), // ← Utilise actual_end_grid
        parent: None,
    };
    open_set.push(start_node.clone());
    all_nodes.insert(start_grid, start_node);

    let mut expansions: usize = 0;

    while let Some(current_node) = open_set.pop() {
        expansions += 1;
        if expansions > max_expansions {
            if return_partial_on_limit {
                if let Some(best) = all_nodes
                    .values()
                    .min_by(|a, b| a.h_cost.partial_cmp(&b.h_cost).unwrap_or(Ordering::Equal))
                {
                    return Some(reconstruct_path(&all_nodes, best.pos));
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }

        if current_node.pos == actual_end_grid {
            // ← Utilise actual_end_grid
            return Some(reconstruct_path(&all_nodes, actual_end_grid));
        }

        for neighbor_pos in get_neighbors(current_node.pos) {
            if is_diagonal(current_node.pos, neighbor_pos) {
                let corner_1 = IVec2 {
                    x: current_node.pos.x,
                    y: neighbor_pos.y,
                };
                let corner_2 = IVec2 {
                    x: neighbor_pos.x,
                    y: current_node.pos.y,
                };

                if !is_tile_passable(corner_1, structure_manager)
                    || !is_tile_passable(corner_2, structure_manager)
                {
                    continue;
                }
            }

            if !is_tile_passable(neighbor_pos, structure_manager) {
                continue;
            }

            let move_cost = if is_diagonal(current_node.pos, neighbor_pos) {
                1.414
            } else {
                1.0
            };
            let new_g_cost = current_node.g_cost + move_cost;

            if let Some(existing_node) = all_nodes.get_mut(&neighbor_pos) {
                if new_g_cost < existing_node.g_cost {
                    existing_node.g_cost = new_g_cost;
                    existing_node.parent = Some(current_node.pos);
                    open_set.push(existing_node.clone());
                }
            } else {
                let neighbor_node = PathNode {
                    pos: neighbor_pos,
                    g_cost: new_g_cost,
                    h_cost: heuristic(neighbor_pos, actual_end_grid), // ← Utilise actual_end_grid
                    parent: Some(current_node.pos),
                };
                open_set.push(neighbor_node.clone());
                all_nodes.insert(neighbor_pos, neighbor_node);
            }
        }
    }

    None
}

// ✅ NOUVELLE FONCTION - trouve la case passable la plus proche en privilégiant la direction d'approche
fn find_nearest_passable_tile(
    target: IVec2,
    start: IVec2,
    structure_manager: &Res<StructureManager>,
) -> Option<IVec2> {
    // Calcule la direction d'approche depuis le point de départ
    let approach_dir = IVec2::new((target.x - start.x).signum(), (target.y - start.y).signum());

    // Liste des directions à tester, en commençant par celle opposée à l'approche
    let mut directions = Vec::new();

    // Direction opposée à l'approche (côté le plus proche)
    if approach_dir.x != 0 || approach_dir.y != 0 {
        directions.push(IVec2::new(-approach_dir.x, -approach_dir.y));
    }

    // Directions perpendiculaires à l'approche
    if approach_dir.x != 0 {
        directions.push(IVec2::new(0, 1)); // Nord
        directions.push(IVec2::new(0, -1)); // Sud
    }
    if approach_dir.y != 0 {
        directions.push(IVec2::new(1, 0)); // Est
        directions.push(IVec2::new(-1, 0)); // Ouest
    }

    // Diagonales (moins prioritaires)
    directions.extend([
        IVec2::new(-1, -1),
        IVec2::new(1, -1),
        IVec2::new(-1, 1),
        IVec2::new(1, 1),
    ]);

    // Direction d'approche en dernier recours
    if approach_dir.x != 0 || approach_dir.y != 0 {
        directions.push(approach_dir);
    }

    // Teste chaque direction par ordre de priorité
    for radius in 1..=5 {
        // Réduit le rayon pour être plus efficace
        for &dir in &directions {
            let candidate = target + dir * radius;
            if is_tile_passable(candidate, structure_manager) {
                return Some(candidate);
            }
        }
    }

    None
}

// ========== SYSTÈMES BEVY ==========

/// Système qui calcule le chemin pour les agents.
pub fn pathfinding_system(
    mut agents_query: Query<(&mut PathfindingAgent, &Transform)>,
    structure_manager: Res<StructureManager>,
) {
    for (mut agent, transform) in agents_query.iter_mut() {
        if let Some(target) = agent.target {
            // Convert transform -> tile pos once
            let start_tile = world_pos_to_tile(transform.translation.xy());
            // Recalculer le chemin si la cible a changé (chemin vide)
            if agent.path.is_empty() {
                if let Some(new_path) = find_path(start_tile, target, &structure_manager) {
                    agent.path = new_path;
                } else {
                    // Impossible de trouver un chemin, on annule la cible.
                    agent.target = None;
                }
            }
        }
    }
}

/// makes the entiry moves along the path ; sets CurrentTask.0 to None when reached target
pub fn movement_system(
    mut agents_query: Query<(
        &mut PathfindingAgent,
        &MovementSpeed,
        &mut Transform,
        &mut CurrentTask,
    )>,
    time: Res<Time>,
) {
    for (mut agent, movement_speed, mut transform, mut current_task) in agents_query.iter_mut() {
        if let Some(&next_waypoint) = agent.path.front() {
            let current_tile_pos = world_pos_to_tile(transform.translation.xy());
            let distance = current_tile_pos.distance(next_waypoint);

            if distance <= agent.path_tolerance {
                // Waypoint atteint, on le retire et passe au suivant
                agent.path.pop_front();
                if agent.path.is_empty() {
                    agent.target = None; // Destination finale atteinte
                    current_task.task = None;
                }
            } else {
                // Se déplacer vers le waypoint
                let direction = (next_waypoint - current_tile_pos).normalize_or_zero();

                // move in pixels: convert tile movement to pixels
                let movement_tiles = direction * movement_speed.0 * time.delta_secs();
                let movement_pixels =
                    movement_tiles * Vec2::new(map::TILE_SIZE.x, map::TILE_SIZE.y);
                transform.translation.x += movement_pixels.x;
                transform.translation.y += movement_pixels.y;

                // Rotation du sprite
                if direction != Vec2::ZERO {
                    let angle = direction.y.atan2(direction.x) - std::f32::consts::FRAC_PI_2;
                    transform.rotation = Quat::from_rotation_z(angle);
                }
            }
        }
    }
}

/// Système pour définir une cible avec le clic droit de la souris.
fn mouse_target_system(
    mut agents_query: Query<&mut PathfindingAgent>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform)>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let Some((camera, camera_transform)) = cameras.iter().next() else {
        return;
    };

    if let Some(cursor_pos) = window.cursor_position() {
        if let Ok(world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos) {
            let tile_pos = world_pos_to_tile(world_pos);
            for mut agent in agents_query.iter_mut() {
                agent.target = Some(tile_pos);
                agent.path.clear(); // Force le recalcul du chemin
            }
        }
    }
}

impl Plugin for PathfindingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                pathfinding_system,
                movement_system,
                mouse_target_system.run_if(input_just_pressed(MouseButton::Right)),
            )
                .chain(),
        );
    }
}
