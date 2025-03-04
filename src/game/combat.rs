use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        system::{Commands, Res, ResMut},
    },
    time::Timer,
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{ActiveEvents, Collider};
use rand::Rng;
use std::time::Duration;
use strum::EnumDiscriminants;

use crate::{game::collision::CollisionGroupSet, server::ApplicationCtx, Direction};

use super::pawns::Player;

#[derive(Debug, Clone)]
pub struct Combo {
    pub combo_counter: u32,
    pub combo_timer: Timer,
}

impl Default for Combo {
    /// Creates a new [`Combo`] instance, with a default duration and with the [`bevy::time::TimerMode::Once`] mode.
    /// This means that this Timer will have a duration of 0s.
    fn default() -> Self {
        Self {
            combo_counter: 0,
            combo_timer: Timer::new(Duration::default(), bevy::time::TimerMode::Once),
        }
    }
}

impl Combo {
    pub fn new(duration: Duration) -> Self {
        Self {
            combo_counter: 0,
            combo_timer: Timer::new(duration, bevy::time::TimerMode::Once),
        }
    }
}

#[derive(Component, Clone)]
pub struct AttackObject {
    pub attack_origin: Transform,
    pub attack_type: AttackType,
    pub attack_strength: f32,
    pub attack_by: Entity,
}

impl AttackObject {
    pub fn new(
        attack_type: AttackType,
        attack_strength: f32,
        attack_origin: Transform,
        attack_by: Entity,
    ) -> Self {
        Self {
            attack_origin,
            attack_type,
            attack_strength,
            attack_by,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackType {
    Directional(Direction),
    Super,
    Quick,
}

#[derive(Clone)]
/// A special effect, which can affect any [`Player`]s and subsets of the instnace.
/// These effects influence the players ability to perform in the game.
pub struct Effect {
    /// The type of the effect.
    pub effect_type: EffectType,
    /// The duration of the effect, if the duration is [`None`], it means it presistent and does not expire.
    pub duration: Option<Timer>,
}

impl Effect {
    pub fn new(effect_type: EffectType, duration: Option<Timer>) -> Self {
        Self {
            effect_type,
            duration,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, EnumDiscriminants)]
pub enum EffectType {
    Slowdown,
    Stunned,
}

/// Spawns in a Cuboid and then the collisions are checked so that we know which enemies are affected.
pub fn spawn_attack(
    mut commands: Commands<'_, '_>,
    collision_groups: Res<'_, CollisionGroupSet>,
    mut app_ctx: ResMut<'_, ApplicationCtx>,
    entity: Entity,
    local_player: &mut Player,
    transform: &Transform,
    attack_collider: Collider,
    attack_transform: Transform,
) {
    commands
        .spawn(attack_collider)
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(ActiveEvents::CONTACT_FORCE_EVENTS)
        .insert(AttackObject::new(
            AttackType::Directional(local_player.direction),
            app_ctx.rand.random_range(14.0..21.0),
            *transform,
            entity,
        ))
        .insert(collision_groups.attack_obj)
        .insert(attack_transform);
}
