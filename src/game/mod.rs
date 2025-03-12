use bevy::ecs::system::Resource;
use rand::{rngs::SmallRng, SeedableRng};

pub mod collision;
pub mod combat;
pub mod pawns;

/// This [`RandomEngine`] should never be used in crypto cases, as it uses a [`SmallRng`] in inside.
/// The struct has been purely created for making a Rng a [`Resource`] for bevy.
#[derive(Resource)]
pub struct RandomEngine {
    pub inner: SmallRng,
}

impl Default for RandomEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RandomEngine {
    pub fn new() -> Self {
        Self {
            inner: SmallRng::from_rng(&mut rand::rng()),
        }
    }
}
