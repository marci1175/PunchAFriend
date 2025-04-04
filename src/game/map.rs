use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        system::{Commands, Query},
    },
    math::{vec2, Vec2},
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{ActiveEvents, Collider};

use super::collision::CollisionGroupSet;

#[derive(Component, Clone)]
/// A MapElement instnace is an object which is a part of the map.
/// This is used to make difference between Entities which are a part of the obstacles contained in the map.
pub struct MapElement;

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct MapObject {
    pub size: Vec2,
    pub position: Vec2,
    pub texture_name: String,
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct MapInstance {
    pub objects: Vec<MapObject>,
}

impl MapInstance {
    pub fn map_flatground() -> Self {
        let mut map_objects: Vec<MapObject> = vec![];

        map_objects.push(MapObject {
            size: vec2(500., 30.),
            position: vec2(0., -200.),
            texture_name: String::new(),
        });

        Self {
            objects: map_objects,
        }
    }

    pub fn map_islands() -> Self {
        let mut map_objects: Vec<MapObject> = vec![];

        for position in (-400..400).step_by(150) {
            map_objects.push(MapObject {
                size: vec2(40., 10.),
                position: vec2(position as f32, -200.),
                texture_name: String::new(),
            });
        }

        Self {
            objects: map_objects,
        }
    }
}

/// The maps' name which the client can vote for in the intermission state, and load in if the vote has been finalized.
#[derive(
    Clone,
    Debug,
    serde::Deserialize,
    serde::Serialize,
    strum::EnumDiscriminants,
    strum::Display,
    strum::EnumCount,
)]
#[strum_discriminants(derive(
    serde::Deserialize,
    serde::Serialize,
    strum::Display,
    strum::VariantArray
))]
pub enum MapName {
    #[strum(to_string = "FlatGround")]
    /// The original map. Consists of one rectangluar brick in the middle.
    FlatGround(MapInstance),

    #[strum(to_string = "Islands")]
    /// The original map. Consists of one rectangluar brick in the middle.
    Islands(MapInstance),
}

impl MapNameDiscriminants {
    pub fn into_map_instance(&self) -> MapInstance {
        match self {
            MapNameDiscriminants::FlatGround => MapInstance::map_flatground(),
            MapNameDiscriminants::Islands => MapInstance::map_islands(),

            _ => unimplemented!(),
        }
    }
}

/// Loads entites in from a [`MapInstance`], this is used to load in maps provided by servers.
pub fn load_map_from_mapinstance(
    map_instance: MapInstance,
    commands: &mut Commands,
    collision_groups: CollisionGroupSet,
    current_game_objects: Query<(Entity, &MapElement)>,
    // meshes: ResMut<Assets<Mesh>>,
    // materials: ResMut<Assets<ColorMaterial>>,
    // mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // Delete all currently existing map objects.
    for (entity, _game_object) in &current_game_objects {
        commands.entity(entity).despawn();
    }

    for object in map_instance.objects {
        commands
            .spawn(Collider::cuboid(object.size.x, object.size.y))
            .insert(Transform::from_xyz(
                object.position.x,
                object.position.y,
                0.,
            ))
            .insert(ActiveEvents::COLLISION_EVENTS)
            .insert(collision_groups.map_object)
            .insert(MapElement);
    }
}
