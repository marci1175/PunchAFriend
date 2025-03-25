use bevy::{ecs::component::Component, math::Vec2};

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

/// The maps' name which the client can vote for in the intermission state, and load in if the vote has been finalized.
#[derive(
    Clone, Debug, serde::Deserialize, serde::Serialize, strum::EnumDiscriminants, strum::Display,
)]
#[strum_discriminants(derive(serde::Deserialize, serde::Serialize))]
pub enum MapName {
    #[strum(to_string = "FlatGround")]
    /// The original map. Consists of one rectangluar brick in the middle.
    FlatGround(MapInstance),
}
