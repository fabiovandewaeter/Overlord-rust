use crate::map::{
    self, CHUNK_SIZE, ChunkManager, SolidStructure, Structure, StructureManager, world_pos_to_tile,
};
use crate::units::tasks::CurrentTask;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
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

/// Convertit une position logique (en tuiles) en position de grille pour A*.
pub fn tile_to_grid_pos(tile_pos: Vec2) -> IVec2 {
    IVec2 {
        x: tile_pos.x.floor() as i32,
        y: tile_pos.y.floor() as i32,
    }
}

/// Convertit une position de grille A* en position logique (centre de la tuile).
pub fn grid_to_tile_pos(grid_pos: IVec2) -> Vec2 {
    Vec2::new(grid_pos.x as f32 + 0.5, grid_pos.y as f32 + 0.5)
}

/// Convertit une position globale de tuile en position de chunk.
pub fn global_tile_to_chunk_pos(tile_pos: IVec2) -> IVec2 {
    let chunk_size = CHUNK_SIZE.as_ivec2();
    IVec2::new(
        (tile_pos.x as f32 / chunk_size.x as f32).floor() as i32,
        (tile_pos.y as f32 / chunk_size.y as f32).floor() as i32,
    )
}

/// Convertit une position globale de tuile en position locale dans son chunk.
pub fn global_to_local_tile_pos(tile_pos: IVec2) -> TilePos {
    let chunk_pos = global_tile_to_chunk_pos(tile_pos);
    let chunk_size = CHUNK_SIZE.as_ivec2();
    let local_x = tile_pos.x - chunk_pos.x * chunk_size.x;
    let local_y = tile_pos.y - chunk_pos.y * chunk_size.y;
    TilePos {
        x: local_x as u32,
        y: local_y as u32,
    }
}

// ========== PATHFINDING UTILISANT DIRECTEMENT VOTRE TILEMAP ==========

#[derive(Component)]
pub struct PathfindingAgent {
    pub target: Option<Vec2>,
    pub path: VecDeque<Vec2>,
    // pub current_path_index: usize,
    pub speed: f32,
    pub path_tolerance: f32,
}

/// checks if there is a structure at rounded_tile_pos
pub fn is_tile_passable(
    rounded_tile_pos: IVec2,
    structure_manager: &Res<StructureManager>,
) -> bool {
    if let Some(_structure_entity) = structure_manager.structures.get(&rounded_tile_pos) {
        return false;
    }
    // Si le chunk n'existe pas, on suppose qu'il n'y a pas de mur.
    // TODO: change that or spawn the chunk
    true
}

fn is_diagonal(from: IVec2, to: IVec2) -> bool {
    (from.x - to.x).abs() == 1 && (from.y - to.y).abs() == 1
}

fn heuristic(a: IVec2, b: IVec2) -> f32 {
    let dx = (a.x - b.x) as f32;
    let dy = (a.y - b.y) as f32;
    (dx * dx + dy * dy).sqrt()
}

pub fn get_neighbors(pos: IVec2) -> impl Iterator<Item = IVec2> {
    (-1..=1)
        .flat_map(move |x| (-1..=1).map(move |y| (x, y)))
        .filter(|&(x, y)| x != 0 || y != 0)
        .map(move |(dx, dy)| IVec2 {
            x: pos.x + dx,
            y: pos.y + dy,
        })
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

fn find_path(
    start_pos: Vec2,
    end_pos: Vec2,
    structure_manager: &Res<StructureManager>,
) -> Option<VecDeque<Vec2>> {
    let start_grid = tile_to_grid_pos(start_pos);
    let end_grid = tile_to_grid_pos(end_pos);

    if !is_tile_passable(end_grid, structure_manager) {
        return None;
    }

    // -------------------------
    // Configuration de la limite
    // -------------------------
    // Ajuste ces constantes selon tes besoins.
    const BASE_LIMIT: usize = 500; // nombre d'expansions minimal
    const PER_TILE_LIMIT: usize = 40; // coût par tuile de distance (augmente la limite si distance élevée)
    const MAX_LIMIT: usize = 20_000; // plafond absolu pour éviter explosion

    let dist_tiles = heuristic(start_grid, end_grid); // distance en tuiles (float)
    let per_tile_extra = ((dist_tiles).round() as isize).max(0) as usize;
    let mut max_expansions = BASE_LIMIT + per_tile_extra * PER_TILE_LIMIT;
    if max_expansions > MAX_LIMIT {
        max_expansions = MAX_LIMIT;
    }

    // Si true -> retourner un chemin partiel vers le meilleur noeud exploré quand on atteint la limite.
    // Si false -> retourner None (annuler la cible).
    let return_partial_on_limit = true;

    // -------------------------
    // A* habituel
    // -------------------------
    let mut open_set = BinaryHeap::new();
    let mut all_nodes: HashMap<IVec2, PathNode> = HashMap::new();

    let start_node = PathNode {
        pos: start_grid,
        g_cost: 0.0,
        h_cost: heuristic(start_grid, end_grid),
        parent: None,
    };
    open_set.push(start_node.clone());
    all_nodes.insert(start_grid, start_node);

    let mut expansions: usize = 0;

    while let Some(current_node) = open_set.pop() {
        expansions += 1;
        // Si trop d'expansions, on coupe
        if expansions > max_expansions {
            if return_partial_on_limit {
                // Choisir le noeud exploré ayant le plus petit h_cost (le plus proche heuristiquement de la cible)
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

        if current_node.pos == end_grid {
            return Some(reconstruct_path(&all_nodes, end_grid));
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
                    h_cost: heuristic(neighbor_pos, end_grid),
                    parent: Some(current_node.pos),
                };
                open_set.push(neighbor_node.clone());
                all_nodes.insert(neighbor_pos, neighbor_node);
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
    mut agents_query: Query<(&mut PathfindingAgent, &mut Transform, &mut CurrentTask)>,
    time: Res<Time>,
) {
    for (mut agent, mut transform, mut current_task) in agents_query.iter_mut() {
        if let Some(&next_waypoint) = agent.path.front() {
            let current_tile_pos = world_pos_to_tile(transform.translation.xy());
            let distance = current_tile_pos.distance(next_waypoint);

            if distance <= agent.path_tolerance {
                // Waypoint atteint, on le retire et passe au suivant
                agent.path.pop_front();
                if agent.path.is_empty() {
                    agent.target = None; // Destination finale atteinte
                }
                current_task.0 = None;
            } else {
                // Se déplacer vers le waypoint
                let direction = (next_waypoint - current_tile_pos).normalize_or_zero();

                // move in pixels: convert tile movement to pixels
                let movement_tiles = direction * agent.speed * time.delta_secs();
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
