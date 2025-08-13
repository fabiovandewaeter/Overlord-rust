use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
};
use bevy_ecs_tilemap::prelude::*;
use rand::Rng;

pub const TILE_SIZE: TilemapTileSize = TilemapTileSize { x: 32.0, y: 32.0 };
// For this example, don't choose too large a chunk size.
pub const CHUNK_SIZE: UVec2 = UVec2 { x: 4, y: 4 };
// Render chunk sizes are set to 4 render chunks per user specified chunk.
pub const RENDER_CHUNK_SIZE: UVec2 = UVec2 {
    x: CHUNK_SIZE.x * 2,
    y: CHUNK_SIZE.y * 2,
};
pub const LAYER_LEVEL: f32 = -1.0;

pub struct MapPlugin;

#[derive(Component)]
pub struct Wall;

pub fn spawn_chunk(
    commands: &mut Commands,
    asset_server: &AssetServer,
    chunk_pos: IVec2,
) -> Entity {
    let tilemap_entity = commands.spawn_empty().id();
    let mut tile_storage = TileStorage::empty(CHUNK_SIZE.into());
    let mut rng = rand::rng();

    // Spawn the elements of the tilemap.
    for x in 0..CHUNK_SIZE.x {
        for y in 0..CHUNK_SIZE.y {
            let tile_pos = TilePos { x, y };
            let mut tile_commands = commands.spawn(TileBundle {
                position: tile_pos,
                tilemap_id: TilemapId(tilemap_entity),
                texture_index: TileTextureIndex(0),
                ..Default::default()
            });

            let is_wall = rng.random_bool(0.2);
            if is_wall {
                tile_commands.insert(Wall);
                tile_commands.insert(TileTextureIndex(1));
            }

            let tile_entity = tile_commands.id();
            // commands.entity(tilemap_entity).add_child(tile_entity);
            tile_storage.set(&tile_pos, tile_entity);
        }
    }

    let transform = Transform::from_translation(Vec3::new(
        chunk_pos.x as f32 * CHUNK_SIZE.x as f32 * TILE_SIZE.x + TILE_SIZE.x * 0.5,
        chunk_pos.y as f32 * CHUNK_SIZE.y as f32 * TILE_SIZE.y + TILE_SIZE.y * 0.5,
        LAYER_LEVEL,
    ));

    let image_handles = vec![
        asset_server.load("tiles/grass.png"),
        asset_server.load("tiles/stone.png"),
    ];

    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size: TILE_SIZE.into(),
        size: CHUNK_SIZE.into(),
        storage: tile_storage,
        texture: TilemapTexture::Vector(image_handles),
        tile_size: TILE_SIZE,
        transform,
        render_settings: TilemapRenderSettings {
            render_chunk_size: RENDER_CHUNK_SIZE,
            ..Default::default()
        },
        ..Default::default()
    });
    tilemap_entity
}

// Conversion coordonnées logiques -> monde
pub fn tile_coords_to_world(tile_coords: Vec2) -> Vec2 {
    Vec2::new(tile_coords.x * TILE_SIZE.x, tile_coords.y * TILE_SIZE.y)
}

// Conversion monde -> coordonnées logiques
pub fn world_coords_to_tile(world_coords: Vec2) -> Vec2 {
    Vec2::new(world_coords.x / TILE_SIZE.x, world_coords.y / TILE_SIZE.y)
}

/// Convertit une position monde (pixels) en position de chunk.
pub fn world_pos_to_chunk_pos(world_pos: &Vec2) -> IVec2 {
    let chunk_size_pixels = CHUNK_SIZE.as_vec2() * Vec2::new(TILE_SIZE.x, TILE_SIZE.y);
    let pos = *world_pos / chunk_size_pixels;
    IVec2::new(pos.x.floor() as i32, pos.y.floor() as i32)
}

pub fn camera_pos_to_chunk_pos(camera_pos: &Vec2) -> IVec2 {
    let camera_pos = camera_pos.as_ivec2();
    let chunk_size: IVec2 = IVec2::new(CHUNK_SIZE.x as i32, CHUNK_SIZE.y as i32);
    let tile_size: IVec2 = IVec2::new(TILE_SIZE.x as i32, TILE_SIZE.y as i32);
    camera_pos / (chunk_size * tile_size)
}

/// Convertit une position logique (en tiles) en position de chunk
pub fn tile_pos_to_chunk_pos(tile_pos: Vec2) -> IVec2 {
    let chunk_size_f32 = Vec2::new(CHUNK_SIZE.x as f32, CHUNK_SIZE.y as f32);
    IVec2::new(
        (tile_pos.x / chunk_size_f32.x).floor() as i32,
        (tile_pos.y / chunk_size_f32.y).floor() as i32,
    )
}

fn spawn_chunks_around_camera(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    camera_query: Query<&Transform, With<Camera>>,
    mut chunk_manager: ResMut<ChunkManager>,
) {
    for transform in camera_query.iter() {
        let camera_chunk_pos = world_pos_to_chunk_pos(&transform.translation.xy());
        for y in (camera_chunk_pos.y - 2)..(camera_chunk_pos.y + 2) {
            for x in (camera_chunk_pos.x - 2)..(camera_chunk_pos.x + 2) {
                let chunk_pos = IVec2::new(x, y);
                if !chunk_manager.spawned_chunks.contains_key(&chunk_pos) {
                    let entity = spawn_chunk(&mut commands, &asset_server, chunk_pos);
                    chunk_manager.spawned_chunks.insert(chunk_pos, entity);
                }
            }
        }
    }
}

#[derive(Default, Debug, Resource)]
pub struct ChunkManager {
    pub spawned_chunks: HashMap<IVec2, Entity>,
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_plugins(TilemapPlugin)
            .insert_resource(ChunkManager::default())
            .add_systems(Update, spawn_chunks_around_camera);
    }
}
