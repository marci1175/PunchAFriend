use bevy::{
    ecs::component::Component,
    math::{vec2, Vec2},
};

#[derive(Component, Clone)]
/// A MapElement instnace is an object which is a part of the map.
/// This is used to make difference between Entities which are a part of the obstacles contained in the map.
pub struct MapElement;

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct MapObject {
    pub size: Vec2,
    pub position: Vec2,
    pub texture_name: String,
}

#[derive(Component, Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct MapInstance {
    pub objects: Vec<MapObject>,
    pub name: String,
}

impl MapInstance {
    pub fn original_map() -> Self {
        let mut map_objects: Vec<MapObject> = vec![];

        map_objects.push(MapObject {
            size: vec2(500., 30.),
            position: vec2(0., 300.),
            texture_name: String::new(),
        });

        Self {
            objects: map_objects,
            name: String::from("Original"),
        }
    }
}

/// The maps' name which the client can vote for in the intermission state, and load in if the vote has been finalized.
#[derive(
    Clone, Debug, serde::Deserialize, serde::Serialize, strum::EnumDiscriminants, strum::Display,
)]
#[strum_discriminants(derive(serde::Deserialize, serde::Serialize, strum::Display))]
pub enum MapName {
    #[strum(to_string = "FlatGround")]
    /// The original map. Consists of one rectangluar brick in the middle.
    FlatGround(MapInstance),
}
