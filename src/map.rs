use crate::units::Unit;
use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use rand::Rng;
use std::collections::HashMap;

pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 32.0, y: 32.0 };
pub const CHUNK_SIZE: UVec2 = UVec2 { x: 32, y: 32 };
// Render chunk sizes are set to 4 render chunks per user specified chunk.
pub const RENDER_CHUNK_SIZE: UVec2 = UVec2 {
    x: CHUNK_SIZE.x * 2,
    y: CHUNK_SIZE.y * 2,
};
pub const TILE_LAYER_LEVEL: f32 = -1.0;
pub const STRUCTURE_LAYER_LEVEL: f32 = 0.0;

pub struct MapPlugin;

#[derive(Component)]
pub struct Structure;

#[derive(Component)]
pub struct Chest;

// TODO: delete these two component and do something better
#[derive(Component)]
pub struct Provider;
#[derive(Component)]
pub struct Requester;

#[derive(Component)]
pub struct Wall;

pub fn spawn_chunk(
    commands: &mut Commands,
    asset_server: &AssetServer,
    mut structure_manager: &mut ResMut<StructureManager>,
    chunk_pos: IVec2,
) -> Entity {
    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(CHUNK_SIZE.into());
    let mut rng = rand::rng();

    // Collecte les positions des structures à créer
    let mut structures_to_spawn = Vec::new();

    // Spawn the elements of the tilemap.
    for x in 0..CHUNK_SIZE.x {
        for y in 0..CHUNK_SIZE.y {
            let local_tile_pos = TilePos { x, y };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: local_tile_pos,
                    tilemap_id: TilemapId(tilemap_entity),
                    texture_index: TileTextureIndex(0),
                    ..Default::default()
                })
                .id();

            let is_wall = rng.random_bool(0.2);
            if is_wall
                && (chunk_pos.x > 0 || chunk_pos.x < 0)
                && (chunk_pos.y > 0 || chunk_pos.y < 0)
            {
                let local_tile_pos: IVec2 =
                    IVec2::new(local_tile_pos.x as i32, local_tile_pos.y as i32);
                let rounded_tile_pos = local_tile_pos_to_rounded_tile(local_tile_pos, chunk_pos);
                structures_to_spawn.push(rounded_tile_pos);
            }

            commands.entity(tilemap_entity).add_child(tile_entity);
            tile_storage.set(&local_tile_pos, tile_entity);
        }
    }

    // Calcule la position du tilemap dans le monde
    let rounded_tile_pos = rounded_chunk_pos_to_rounded_tile(&chunk_pos);
    let tilemap_world_pos = rounded_tile_pos_to_world(rounded_tile_pos);
    let tilemap_transform = Transform::from_translation(Vec3::new(
        tilemap_world_pos.x,
        tilemap_world_pos.y,
        TILE_LAYER_LEVEL,
    ));

    let image_handles = vec![
        asset_server.load("tiles/grass.png"),
        asset_server.load("tiles/stone.png"),
    ];

    // Configure le tilemap
    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size: TILE_SIZE.into(),
        size: CHUNK_SIZE.into(),
        storage: tile_storage,
        texture: TilemapTexture::Vector(image_handles),
        tile_size: TILE_SIZE,
        transform: tilemap_transform,
        render_settings: TilemapRenderSettings {
            render_chunk_size: RENDER_CHUNK_SIZE,
            ..Default::default()
        },
        ..Default::default()
    });

    // Spawn les structures APRÈS avoir configuré le tilemap
    // et les attache directement au tilemap
    for rounded_tile_pos in structures_to_spawn {
        let wall_entity = commands
            .spawn((
                Structure,
                Wall,
                Sprite::from_image(asset_server.load("structures/wall.png")),
            ))
            .id();

        spawn_structure_in_chunk(
            commands,
            &wall_entity,
            &mut structure_manager,
            tilemap_entity,
            rounded_tile_pos,
            tilemap_world_pos,
        );
    }

    tilemap_entity
}

fn spawn_structure_in_chunk(
    commands: &mut Commands,
    structure_entity: &Entity,
    structure_manager: &mut ResMut<StructureManager>,
    tilemap_entity: Entity,
    rounded_tile_pos: IVec2,
    tilemap_world_pos: Vec2,
) {
    // Calcule la position absolue de la structure
    let structure_world_pos = rounded_tile_pos_to_world(rounded_tile_pos);

    // Calcule la position RELATIVE au tilemap
    let relative_pos = structure_world_pos - tilemap_world_pos;

    let transform = Transform::from_translation(Vec3::new(
        relative_pos.x,
        relative_pos.y,
        STRUCTURE_LAYER_LEVEL - TILE_LAYER_LEVEL, // Z relatif
    ));

    commands.entity(*structure_entity).insert(transform);

    // Attache la structure au tilemap, pas à une tile individuelle
    commands.entity(tilemap_entity).add_child(*structure_entity);

    // Enregistre la structure dans le manager
    structure_manager
        .structures
        .insert(rounded_tile_pos, *structure_entity);
}

// add transform to structure_entity and add it to structure_manager
pub fn place_structure(
    commands: &mut Commands,
    asset_server: &Res<AssetServer>, // Ajouté pour pouvoir spawner le chunk
    structure_entity: &Entity,
    structure_manager: &mut ResMut<StructureManager>,
    chunk_manager: &mut ResMut<ChunkManager>, // Maintenant mutable
    rounded_tile_pos: IVec2,
) {
    let rounded_chunk_pos = rounded_tile_pos_to_rounded_chunk(rounded_tile_pos);

    // Charger le chunk s'il n'existe pas
    if !chunk_manager
        .spawned_chunks
        .contains_key(&rounded_chunk_pos)
    {
        let entity = spawn_chunk(commands, asset_server, structure_manager, rounded_chunk_pos);
        chunk_manager
            .spawned_chunks
            .insert(rounded_chunk_pos, entity);
    }

    // Maintenant le chunk existe forcément
    if let Some(&tilemap_entity) = chunk_manager.spawned_chunks.get(&rounded_chunk_pos) {
        println!("a");
        let tilemap_world_pos =
            rounded_tile_pos_to_world(rounded_chunk_pos_to_rounded_tile(&rounded_chunk_pos));

        spawn_structure_in_chunk(
            commands,
            structure_entity,
            structure_manager,
            tilemap_entity,
            rounded_tile_pos,
            tilemap_world_pos,
        );
    } else {
        panic!();
    }
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

// ========= coordinates conversion =========
// world_pos = (5.5 * TILE_SIZE.X, 0.5 * TILE_SIZE.y) | tile_pos = (5.5, 0.5) | rounded_tile_pos = (5, 0)

pub fn local_tile_pos_to_rounded_tile(local_tile_pos: IVec2, rounded_chunk_pos: IVec2) -> IVec2 {
    IVec2::new(
        rounded_chunk_pos.x * CHUNK_SIZE.x as i32 + local_tile_pos.x,
        rounded_chunk_pos.y * CHUNK_SIZE.y as i32 + local_tile_pos.y,
    )
}

// Conversion coordonnées logiques -> monde ; (5.5, 0.5) => (5.5 * TILE_SIZE.x, 0.5 * TILE_SIZE.y)
pub fn tile_pos_to_world(tile_pos: Vec2) -> Vec2 {
    Vec2::new(tile_pos.x * TILE_SIZE.x, tile_pos.y * TILE_SIZE.y)
}

// adds 0.5 to coordinates to make entities spawn based on the corner of there sprite and not the center
pub fn rounded_tile_pos_to_world(rounded_tile_pos: IVec2) -> Vec2 {
    Vec2::new(
        rounded_tile_pos.x as f32 * TILE_SIZE.x + 0.5 * TILE_SIZE.x,
        rounded_tile_pos.y as f32 * TILE_SIZE.y + 0.5 * TILE_SIZE.y,
    )
}

// (5.5, 0.5) => (5, 0)
pub fn tile_pos_to_rounded_tile(tile_pos: Vec2) -> IVec2 {
    IVec2::new(tile_pos.x.floor() as i32, tile_pos.y.floor() as i32)
}

// Conversion monde -> coordonnées logiques
pub fn world_pos_to_tile(world_pos: Vec2) -> Vec2 {
    Vec2::new(world_pos.x / TILE_SIZE.x, world_pos.y / TILE_SIZE.y)
}

// Conversion monde -> coordonnées logiques
pub fn world_pos_to_rounded_tile(world_pos: Vec2) -> IVec2 {
    IVec2::new(
        (world_pos.x / TILE_SIZE.x).floor() as i32,
        (world_pos.y / TILE_SIZE.y).floor() as i32,
    )
}

/// Convertit une position monde (pixels) en position de chunk.
pub fn world_pos_to_rounded_chunk(world_pos: &Vec2) -> IVec2 {
    // let chunk_size_pixels = CHUNK_SIZE.as_vec2() * Vec2::new(TILE_SIZE.x, TILE_SIZE.y);
    // let pos = *world_pos / chunk_size_pixels;
    // IVec2::new(pos.x.floor() as i32, pos.y.floor() as i32)
    IVec2::new(
        (world_pos.x / (CHUNK_SIZE.x as f32 * TILE_SIZE.x)).floor() as i32,
        (world_pos.y / (CHUNK_SIZE.y as f32 * TILE_SIZE.y)).floor() as i32,
    )
}

pub fn rounded_chunk_pos_to_rounded_tile(rounded_chunk_pos: &IVec2) -> IVec2 {
    IVec2::new(
        rounded_chunk_pos.x * CHUNK_SIZE.x as i32,
        rounded_chunk_pos.y * CHUNK_SIZE.y as i32,
    )
}

pub fn rounded_tile_pos_to_rounded_chunk(rounded_tile_pos: IVec2) -> IVec2 {
    IVec2::new(
        rounded_tile_pos.x / CHUNK_SIZE.x as i32,
        rounded_tile_pos.y / CHUNK_SIZE.y as i32,
    )
}

pub fn tile_pos_to_rounded_chunk(tile_pos: Vec2) -> IVec2 {
    IVec2::new(
        (tile_pos.x / CHUNK_SIZE.x as f32).floor() as i32,
        (tile_pos.y / CHUNK_SIZE.y as f32).floor() as i32,
    )
}

pub fn camera_pos_to_rounded_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    let camera_pos = camera_pos.as_ivec2();
    let chunk_size: IVec2 = IVec2::new(CHUNK_SIZE.x as i32, CHUNK_SIZE.y as i32);
    let tile_size: IVec2 = IVec2::new(TILE_SIZE.x as i32, TILE_SIZE.y as i32);
    camera_pos / (chunk_size * tile_size)
}
// ==========================================

fn spawn_chunks_around_camera(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    camera_query: Query<&Transform, With<Camera>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut structure_manager: ResMut<StructureManager>,
) {
    for transform in camera_query.iter() {
        let camera_chunk_pos = world_pos_to_rounded_chunk(&transform.translation.xy());
        for y in (camera_chunk_pos.y - 2)..(camera_chunk_pos.y + 2) {
            for x in (camera_chunk_pos.x - 2)..(camera_chunk_pos.x + 2) {
                let chunk_pos = IVec2::new(x, y);
                if !chunk_manager.spawned_chunks.contains_key(&chunk_pos) {
                    let entity = spawn_chunk(
                        &mut commands,
                        &asset_server,
                        &mut structure_manager,
                        chunk_pos,
                    );
                    chunk_manager.spawned_chunks.insert(chunk_pos, entity);
                }
            }
        }
    }
}

fn spawn_chunks_around_units(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    unit_query: Query<&Transform, With<Unit>>,
    mut chunk_manager: ResMut<ChunkManager>,
    mut structure_manager: ResMut<StructureManager>,
) {
    // for transform in camera_query.iter() {
    for unit_transform in unit_query {
        let camera_chunk_pos = camera_pos_to_rounded_chunk_pos(&unit_transform.translation.xy());
        for y in (camera_chunk_pos.y - 2)..(camera_chunk_pos.y + 2) {
            for x in (camera_chunk_pos.x - 2)..(camera_chunk_pos.x + 2) {
                let chunk_pos = IVec2::new(x, y);
                if !chunk_manager.spawned_chunks.contains_key(&IVec2::new(x, y)) {
                    let entity = spawn_chunk(
                        &mut commands,
                        &asset_server,
                        &mut structure_manager,
                        chunk_pos,
                    );
                    chunk_manager.spawned_chunks.insert(chunk_pos, entity);
                }
            }
        }
    }
}

#[derive(Resource, Default, Debug)]
pub struct ChunkManager {
    pub spawned_chunks: HashMap<IVec2, Entity>, // rounded_chunk_pos -> chunk
}

/// to quickly find the Structure at coordinates without checking every Structure
#[derive(Resource, Default, Debug)]
pub struct StructureManager {
    pub structures: HashMap<IVec2, Entity>, // rounded_tile_pos -> structure
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_plugins(TilemapPlugin)
            .insert_resource(ChunkManager::default())
            .insert_resource(StructureManager::default())
            .add_systems(
                Update,
                (spawn_chunks_around_camera, spawn_chunks_around_units),
            );
    }
}
