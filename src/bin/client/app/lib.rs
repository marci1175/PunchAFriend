use std::time::Duration;

use bevy::{
    ecs::{component::Component, system::Res},
    time::{Time, Timer},
};

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
/// This serves as a way to manage the state of the entities' animation.
/// This does not contain any textures, this is just a counter to keep track of the index.
pub struct AnimationState {
    /// The built-in timer help keep track of the timing of the animations.
    pub timer: Timer,

    /// The type of the ongoing animation, like jumping, walking, etc.
    pub animation_type: usize,

    /// The state of the ongoing animation, like which animated frame of an animated jump.
    pub animation_idx: usize,

    /// The index of the last animation state.
    pub animation_idx_max: usize,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            timer: Timer::new(Duration::from_secs(1), bevy::time::TimerMode::Repeating),
            animation_type: 0,
            animation_idx: 0,
            animation_idx_max: 0,
        }
    }
}

impl AnimationState {
    /// Creates a new [`AnimationState`] instance.
    pub fn new(timer: Timer, animation_idx_max: usize) -> Self {
        Self {
            timer,
            animation_type: 0,
            animation_idx: 0,
            animation_idx_max,
        }
    }

    /// Modifies the state of the animation on the current [`AnimationState`] instance.
    /// This function needs to be called every frame, as it also handles the incrementation of the inner timer.
    /// Returns the current index of the animation's state.
    pub fn animate_state(&mut self, time: Duration) -> usize {
        self.timer.tick(time);

        self.animation_idx += self.timer.times_finished_this_tick() as usize;

        if self.animation_idx_max <= self.animation_idx {
            self.animation_idx %= self.animation_idx_max;
        }

        self.animation_idx
    }
}
