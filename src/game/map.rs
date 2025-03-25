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