use crate::map::{CHUNK_SIZE, TILE_SIZE, Wall};
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};

pub struct PathfindingPlugin;

#[derive(Component)]
pub struct PathfindingAgent {
    pub target: Option<Vec2>,
    pub path: VecDeque<Vec2>,
    pub current_path_index: usize,
    pub speed: f32,
    pub path_tolerance: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GridPos {
    x: i32,
    y: i32,
}

#[derive(Clone)]
struct PathNode {
    pos: GridPos,
    g_cost: f32,
    h_cost: f32,
    parent: Option<GridPos>,
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

/// Convertit une position monde en position de tile
fn world_to_tile_pos(world_pos: Vec2) -> GridPos {
    GridPos {
        x: (world_pos.x / TILE_SIZE.x).floor() as i32,
        y: (world_pos.y / TILE_SIZE.y).floor() as i32,
    }
}

/// Convertit une position de tile en position monde (centre de la tile)
fn tile_pos_to_world(grid_pos: GridPos) -> Vec2 {
    Vec2::new(
        grid_pos.x as f32 * TILE_SIZE.x + TILE_SIZE.x * 0.5,
        grid_pos.y as f32 * TILE_SIZE.y + TILE_SIZE.y * 0.5,
    )
}

/// Détermine dans quel chunk se trouve une position de tile
fn tile_pos_to_chunk_pos(tile_pos: GridPos) -> IVec2 {
    let chunk_size_i32 = IVec2::new(CHUNK_SIZE.x as i32, CHUNK_SIZE.y as i32);
    IVec2::new(
        if tile_pos.x >= 0 {
            tile_pos.x / chunk_size_i32.x
        } else {
            (tile_pos.x + 1) / chunk_size_i32.x - 1
        },
        if tile_pos.y >= 0 {
            tile_pos.y / chunk_size_i32.y
        } else {
            (tile_pos.y + 1) / chunk_size_i32.y - 1
        },
    )
}

/// Convertit une position de tile globale en position locale dans son chunk
fn tile_pos_to_local_tile_pos(tile_pos: GridPos) -> TilePos {
    let chunk_pos = tile_pos_to_chunk_pos(tile_pos);
    let chunk_size_i32 = IVec2::new(CHUNK_SIZE.x as i32, CHUNK_SIZE.y as i32);

    let local_x = tile_pos.x - chunk_pos.x * chunk_size_i32.x;
    let local_y = tile_pos.y - chunk_pos.y * chunk_size_i32.y;

    TilePos {
        x: local_x as u32,
        y: local_y as u32,
    }
}

// ========== PATHFINDING UTILISANT DIRECTEMENT VOTRE TILEMAP ==========

/// Vérifie si une position de tile est passable en interrogeant directement vos tilemaps
fn is_tile_passable(
    tile_pos: GridPos,
    wall_query: &Query<(&TilePos, &TilemapId), With<Wall>>,
    tilemap_query: &Query<(&Transform, &TileStorage), With<TileStorage>>,
) -> bool {
    // Déterminer le chunk et la position locale
    let chunk_pos = tile_pos_to_chunk_pos(tile_pos);
    let local_tile_pos = tile_pos_to_local_tile_pos(tile_pos);

    // Chercher le tilemap correspondant à ce chunk
    for (tilemap_transform, tile_storage) in tilemap_query.iter() {
        // Vérifier si cette tilemap correspond à notre chunk
        let expected_world_pos = Vec3::new(
            chunk_pos.x as f32 * CHUNK_SIZE.x as f32 * TILE_SIZE.x,
            chunk_pos.y as f32 * CHUNK_SIZE.y as f32 * TILE_SIZE.y,
            -1.0,
        );

        // Si la position de la tilemap correspond à notre chunk
        if (tilemap_transform.translation - expected_world_pos).length() < 1.0 {
            // Récupérer l'entité de la tile à cette position
            if let Some(tile_entity) = tile_storage.get(&local_tile_pos) {
                // Vérifier si cette tile a le composant Wall
                for (tile_pos_component, _) in wall_query.iter() {
                    if tile_pos_component == &local_tile_pos {
                        return false; // C'est un mur, pas passable
                    }
                }
            }
            return true; // Tile trouvée et pas de mur
        }
    }

    // Si on ne trouve pas le chunk, on considère que c'est passable
    // (sera généré plus tard par votre système de chunks)
    true
}

/// Algorithme A* qui utilise directement vos tilemaps existantes
fn find_path(
    start_world: Vec2,
    end_world: Vec2,
    wall_query: &Query<(&TilePos, &TilemapId), With<Wall>>,
    tilemap_query: &Query<(&Transform, &TileStorage), With<TileStorage>>,
) -> Option<VecDeque<Vec2>> {
    let start_grid = world_to_tile_pos(start_world);
    let end_grid = world_to_tile_pos(end_world);

    if !is_tile_passable(end_grid, wall_query, tilemap_query) {
        return None;
    }

    println!("test1");
    let mut open_set = BinaryHeap::new();
    let mut closed_set = std::collections::HashSet::new();
    let mut came_from = HashMap::new();

    let start_node = PathNode {
        pos: start_grid,
        g_cost: 0.0,
        h_cost: heuristic(start_grid, end_grid),
        parent: None,
    };

    open_set.push(start_node);

    while let Some(current) = open_set.pop() {
        if current.pos == end_grid {
            return Some(reconstruct_path(&came_from, current.pos, start_grid));
        }

        if closed_set.contains(&current.pos) {
            continue;
        }
        closed_set.insert(current.pos);

        // Explorer les 8 voisins
        for neighbor_pos in get_neighbors(current.pos) {
            if !is_tile_passable(neighbor_pos, wall_query, tilemap_query)
                || closed_set.contains(&neighbor_pos)
            {
                continue;
            }

            let move_cost = if is_diagonal_move(current.pos, neighbor_pos) {
                1.414 // sqrt(2)
            } else {
                1.0
            };

            let tentative_g = current.g_cost + move_cost;

            let neighbor_node = PathNode {
                pos: neighbor_pos,
                g_cost: tentative_g,
                h_cost: heuristic(neighbor_pos, end_grid),
                parent: Some(current.pos),
            };

            came_from.insert(neighbor_pos, current.pos);
            open_set.push(neighbor_node);
        }
    }

    None
}

fn heuristic(a: GridPos, b: GridPos) -> f32 {
    let dx = (a.x - b.x) as f32;
    let dy = (a.y - b.y) as f32;
    (dx * dx + dy * dy).sqrt()
}

fn get_neighbors(pos: GridPos) -> Vec<GridPos> {
    vec![
        GridPos {
            x: pos.x + 1,
            y: pos.y,
        },
        GridPos {
            x: pos.x - 1,
            y: pos.y,
        },
        GridPos {
            x: pos.x,
            y: pos.y + 1,
        },
        GridPos {
            x: pos.x,
            y: pos.y - 1,
        },
        GridPos {
            x: pos.x + 1,
            y: pos.y + 1,
        },
        GridPos {
            x: pos.x - 1,
            y: pos.y + 1,
        },
        GridPos {
            x: pos.x + 1,
            y: pos.y - 1,
        },
        GridPos {
            x: pos.x - 1,
            y: pos.y - 1,
        },
    ]
}

fn is_diagonal_move(from: GridPos, to: GridPos) -> bool {
    (from.x - to.x).abs() == 1 && (from.y - to.y).abs() == 1
}

fn reconstruct_path(
    came_from: &HashMap<GridPos, GridPos>,
    mut current: GridPos,
    start: GridPos,
) -> VecDeque<Vec2> {
    let mut path = VecDeque::new();

    while current != start {
        path.push_front(tile_pos_to_world(current));
        if let Some(&parent) = came_from.get(&current) {
            current = parent;
        } else {
            break;
        }
    }

    path
}

// ========== SYSTÈMES BEVY ==========

/// Système principal de pathfinding qui utilise directement vos tilemaps
pub fn pathfinding_system(
    mut agents_query: Query<(&mut PathfindingAgent, &Transform)>,
    wall_query: Query<(&TilePos, &TilemapId), With<Wall>>,
    tilemap_query: Query<(&Transform, &TileStorage), With<TileStorage>>,
) {
    for (mut agent, transform) in agents_query.iter_mut() {
        if let Some(target) = agent.target {
            if agent.path.is_empty() {
                // Calculer un nouveau chemin en utilisant directement vos tilemaps
                if let Some(new_path) = find_path(
                    transform.translation.xy(),
                    target,
                    &wall_query,
                    &tilemap_query,
                ) {
                    agent.path = new_path;
                    agent.current_path_index = 0;
                    println!("Chemin calculé avec {} waypoints", agent.path.len());
                } else {
                    println!(
                        "Impossible de trouver un chemin de {:?} vers {:?}",
                        transform.translation.xy(),
                        target
                    );
                }
            }
        }
    }
}

/// Système de mouvement le long du chemin calculé
pub fn movement_system(
    mut agents_query: Query<(&mut PathfindingAgent, &mut Transform)>,
    time: Res<Time>,
) {
    for (mut agent, mut transform) in agents_query.iter_mut() {
        if !agent.path.is_empty() && agent.current_path_index < agent.path.len() {
            let current_pos = transform.translation.xy();

            if let Some(&next_waypoint) = agent.path.get(agent.current_path_index) {
                let direction = (next_waypoint - current_pos).normalize_or_zero();
                let distance_to_waypoint = current_pos.distance(next_waypoint);

                if distance_to_waypoint <= agent.path_tolerance {
                    // Waypoint atteint, passer au suivant
                    agent.current_path_index += 1;
                    if agent.current_path_index >= agent.path.len() {
                        // Chemin terminé
                        agent.path.clear();
                        agent.current_path_index = 0;
                        agent.target = None;
                        println!("Destination atteinte !");
                    }
                } else {
                    // Se déplacer vers le waypoint
                    let movement = direction * agent.speed * time.delta_secs();
                    transform.translation.x += movement.x;
                    transform.translation.y += movement.y;

                    // Rotation vers la direction de mouvement
                    if direction != Vec2::ZERO {
                        let target_angle =
                            direction.y.atan2(direction.x) - std::f32::consts::FRAC_PI_2;
                        transform.rotation = Quat::from_rotation_z(target_angle);
                    }
                }
            }
        }
    }
}

/// Système pour définir une cible avec le clic droit de la souris
pub fn mouse_target_system(
    mut agents_query: Query<&mut PathfindingAgent>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
) {
    if mouse_input.just_pressed(MouseButton::Right) {
        let window = windows.single().unwrap();

        if let Ok((camera, camera_transform)) = cameras.get_single() {
            if let Some(cursor_pos) = window.cursor_position() {
                if let Ok(mut world_pos) = camera.viewport_to_world_2d(camera_transform, cursor_pos)
                {
                    let tile_size: Vec2 = Vec2::new(TILE_SIZE.x as f32, TILE_SIZE.y as f32);
                    world_pos = world_pos / tile_size;
                    world_pos.x += 0.5;
                    world_pos.y += 0.5;
                    println!("Nouvelle cible définie : {:?}", world_pos);

                    // Assigner cette cible à toutes les unités avec PathfindingAgent
                    for mut agent in agents_query.iter_mut() {
                        agent.target = Some(world_pos);
                        agent.path.clear(); // Force le recalcul du chemin
                    }
                }
            }
        }
    }
}

impl Plugin for PathfindingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (pathfinding_system, movement_system, mouse_target_system).chain(),
        );
    }
}
