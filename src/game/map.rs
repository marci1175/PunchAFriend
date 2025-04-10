use std::time::Duration;

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        query::Without,
        system::{Commands, Query},
    },
    math::{vec2, Vec2},
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{ActiveEvents, Ccd, Collider, Friction, Restitution};
use uuid::Uuid;

use super::{collision::CollisionGroupSet, pawns::Pawn};

/// A StaticMapElement instnace is an object which is a part of the map.
/// This is used to make difference between Entities which are a part of the obstacles contained in the map.
#[derive(Component, Clone)]
pub struct MapElement {
    pub id: Uuid,
    pub object_type: ObjectType,
    pub initial_position: Option<Vec2>,
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct VariableObject {
    pub movement_type: ObjectMovement,
    pub movement_state: MovementState,
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub enum MovementState {
    In,
    Out,
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct ObjectMovementParam {
    pub starting_pos: Vec2,
    pub destination_pos: Vec2,

    pub duration: Duration,
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub enum ObjectMovement {
    Circular(Option<Box<ObjectMovement>>, movement_parameters::Circular),
    Linear(Option<Box<ObjectMovement>>, movement_parameters::Linear),
}

pub mod movement_parameters {
    use std::time::Duration;

    use bevy::math::Vec2;

    #[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
    pub struct Linear {
        /// The destination coordinates where this [`super::VariableObject`] is headed to.
        /// The delta movement is caluclated by the subtraction of the destination and the starting position divided by the time we have.
        pub destination_pos: Vec2,

        /// The amount of time it is going to take for the object to reach the `destination_pos`.
        pub duration: Duration,
    }

    #[derive(Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
    pub struct Circular {
        pub center_pos: Vec2,

        pub radius: f32,

        pub angle: f32,

        /// The amount of time it is going to take for the object make a full circle.
        pub duration: Duration,
    }
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct MapObject {
    pub id: Uuid,
    pub size: Vec2,
    pub position: Vec2,
    pub texture_name: String,

    pub object_type: ObjectType,
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct MapObjectUpdate {
    pub transform: Transform,
    pub id: Uuid,
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub enum ObjectType {
    Static,
    Variable(VariableObject),
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct MapInstance {
    pub objects: Vec<MapObject>,
}

impl MapInstance {
    pub fn map_flatground() -> Self {
        let mut map_objects: Vec<MapObject> = vec![];

        map_objects.push(MapObject {
            id: Uuid::new_v4(),
            size: vec2(500., 30.),
            position: vec2(0., -200.),
            texture_name: String::new(),
            object_type: ObjectType::Static,
        });

        Self {
            objects: map_objects,
        }
    }

    pub fn map_islands() -> Self {
        let mut map_objects: Vec<MapObject> = vec![];

        for position in (-400..400).step_by(150) {
            map_objects.push(MapObject {
                id: Uuid::new_v4(),
                size: vec2(40., 10.),
                position: vec2(position as f32, -200.),
                texture_name: String::new(),
                object_type: ObjectType::Static,
            });
        }

        Self {
            objects: map_objects,
        }
    }

    pub fn map_test() -> Self {
        let mut map_objects: Vec<MapObject> = vec![];

        map_objects.push(MapObject {
            id: Uuid::new_v4(),
            size: vec2(500., 30.),
            position: vec2(0., -200.),
            texture_name: String::new(),
            object_type: ObjectType::Static,
        });

        map_objects.push(MapObject {
            id: Uuid::new_v4(),
            size: vec2(20., 50.),
            position: vec2(300., -200.),
            texture_name: String::new(),
            object_type: ObjectType::Variable(VariableObject {
                movement_state: MovementState::In,
                movement_type: ObjectMovement::Linear(
                    None,
                    movement_parameters::Linear {
                        destination_pos: vec2(350., -200.),
                        duration: Duration::from_secs(2),
                    },
                ),
            }),
        });

        map_objects.push(MapObject {
            id: Uuid::new_v4(),
            size: vec2(50., 20.),
            position: vec2(300., -200.),
            texture_name: String::new(),
            object_type: ObjectType::Variable(VariableObject {
                movement_state: MovementState::In,
                movement_type: ObjectMovement::Circular(
                    None,
                    movement_parameters::Circular {
                        center_pos: vec2(300., 0.),
                        radius: 50.,
                        angle: 0.0,
                        duration: Duration::from_secs_f32(4.0),
                    },
                ),
            }),
        });

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
    current_game_objects: Query<(Entity, &MapElement, &mut Transform), Without<Pawn>>,
    // meshes: ResMut<Assets<Mesh>>,
    // materials: ResMut<Assets<ColorMaterial>>,
    // mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // Delete all currently existing map objects.
    for (entity, _game_object, _) in &current_game_objects {
        commands.entity(entity).despawn();
    }

    for object in map_instance.objects {
        commands
            .spawn(bevy_rapier2d::prelude::RigidBody::KinematicPositionBased)
            .insert(Collider::cuboid(object.size.x, object.size.y))
            .insert(Transform::from_xyz(
                object.position.x,
                object.position.y,
                0.,
            ))
            .insert(ActiveEvents::COLLISION_EVENTS)
            .insert(collision_groups.map_object)
            .insert(Ccd::enabled())
            .insert(Restitution::coefficient(0.))
            .insert(Friction::coefficient(1.))
            .insert(MapElement {
                object_type: object.object_type,
                id: object.id,
                initial_position: Some(object.position),
            });
    }
}
