use std::time::Duration;

use bevy::{ecs::component::Component, time::Timer, transform::components::Transform};

#[derive(Debug, Component, Default)]
/// This struct serves as a way for clients to keep track of the other players' ticks.
/// Every entity has this attribute so that the client knows whether the message sent by the client is the latest one.
pub struct UniqueLastTickCount(u64);

impl UniqueLastTickCount {
    /// Create a new instance, with a pre-set inner value.
    pub fn new(tick: u64) -> Self {
        Self(tick)
    }

    /// Modify the inner value of this [`UniqueLastTickCount`] instance.
    pub fn with_tick(&mut self, new_tick: u64) {
        self.0 = new_tick;
    }

    /// Returns the inner value of this [`UniqueLastTickCount`] instance.
    pub fn get_inner(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Component, Clone)]
/// This serves as a way to manage the state of the entities' animations.
/// This does not contain any textures, this is just a counter to keep track of the index.
pub struct AnimationState {
    /// The built-in timer help keep track of the timing of the animations.
    pub timer: Timer,

    /// The state of the ongoing animation, like which animated frame of an animated jump.
    pub animation_idx: usize,

    /// The index of the last animation state.
    pub animation_idx_max: usize,
}

impl AnimationState {
    /// Creates a new [`AnimationState`] instance.
    pub fn new(timer: Timer, animation_idx_max: usize, animation_idx_begin: usize) -> Self {
        Self {
            timer,
            // animation_type,
            animation_idx: animation_idx_begin,
            animation_idx_max,
        }
    }

    pub fn set_idx_max(&mut self, max: usize) {
        self.animation_idx_max = max;
    }

    pub fn set_current_idx(&mut self, idx: usize) {
        self.animation_idx = idx;
    }

    /// Modifies the state of the animation on the current [`AnimationState`] instance.
    /// This function needs to be called every frame, as it also handles the incrementation of the inner timer.
    /// Returns the current index of the animation's state.
    pub fn animate_state(&mut self, time: Duration) -> usize {
        self.timer.tick(time);

        self.animation_idx += self.timer.times_finished_this_tick() as usize;

        self.animation_idx = self
            .animation_idx
            .checked_rem(self.animation_idx_max)
            .unwrap_or_default();

        self.animation_idx
    }
}

#[derive(Debug, Default, Clone, Component)]
pub struct LastTransformState(Transform);

impl LastTransformState {
    pub fn new(inner: Transform) -> Self {
        Self(inner)
    }

    pub fn set_inner(&mut self, inner: Transform) {
        self.0 = inner;
    }

    pub fn get_inner(&self) -> &Transform {
        &self.0
    }
}
