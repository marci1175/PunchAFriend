use std::ops::Deref;

use bevy::ecs::{component::Component, system::Resource};

#[derive(Debug, Component)]
pub struct UniqueLastTickCount(u64);

impl UniqueLastTickCount {
    pub fn new(tick: u64) -> Self {
        Self(tick)
    }

    pub fn with_tick(&mut self, new_tick: u64) {
        self.0 = new_tick;
    }

    pub fn get_inner(&self) -> u64 {
        self.0
    }
}