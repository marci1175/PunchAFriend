use bevy::{ecs::system::Resource, time::Timer};

#[derive(Resource)]
pub struct NetworkTimer(pub Timer);

impl NetworkTimer {
    pub fn new(timer: Timer) -> Self {
        Self(timer)
    }
}