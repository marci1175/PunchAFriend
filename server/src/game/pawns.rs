use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        system::{Commands, Res, ResMut},
        world::Mut,
    },
    input::{keyboard::KeyCode, ButtonInput},
    math::vec2,
    time::Time,
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController, Velocity};
use std::time::Duration;

use crate::{ApplicationCtx, CollisionGroupSet, Direction};

use super::combat::{spawn_attack, Combo, Effect, EffectType};

#[derive(Component, Clone)]
pub struct LocalPlayer {
    pub name: String,
    pub jumps_remaining: u8,
    pub direction: Direction,
    pub player: Player,
    pub combo_stats: Option<Combo>,
}

impl Default for LocalPlayer {
    fn default() -> Self {
        Self {
            name: String::new(),
            jumps_remaining: 2,
            direction: Direction::default(),
            player: Player::default(),
            combo_stats: None,
        }
    }
}

impl LocalPlayer {
    pub fn new(
        name: String,
        jumps_remaining: u8,
        direction: Direction,
        player: Player,
        combo_counter: Option<Combo>,
    ) -> Self {
        Self {
            name,
            jumps_remaining,
            direction,
            player,
            combo_stats: combo_counter,
        }
    }
}

/// This function modifies the direction variable of the `LocalPlayer`, the variable is always the key last pressed by the user.
pub fn set_movement_direction_var(
    keyboard_input: &ButtonInput<KeyCode>,
    local_player: &mut Mut<'_, LocalPlayer>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyD) {
        // Update latest direction
        local_player.direction = Direction::Right;
    }

    if keyboard_input.just_pressed(KeyCode::KeyA) {
        // Update latest direction
        local_player.direction = Direction::Left;
    }

    if keyboard_input.just_pressed(KeyCode::KeyW) {
        // Update latest direction
        local_player.direction = Direction::Up;
    }
}

/// Handles the local player's input and modifying the controller of the Entity according to the input given.
pub fn local_player_movement(
    commands: &mut Commands<'_, '_>,
    keyboard_input: &ButtonInput<KeyCode>,
    time: &Res<'_, Time>,
    entity: Entity,
    local_player: &mut Mut<'_, LocalPlayer>,
    mut controller: Mut<'_, KinematicCharacterController>,
) {
    let move_factor = 450. * {
        if local_player.player.has_effect(EffectType::Slowdown) {
            0.5
        }
        else {
            1.    
        }
    };

    if keyboard_input.pressed(KeyCode::KeyA) {
        // Move the local player to the left
        controller.translation = Some(vec2(-move_factor * time.delta_secs(), 0.));
    }

    if keyboard_input.pressed(KeyCode::KeyD) {
        // Move the local player to the right
        controller.translation = Some(vec2(move_factor * time.delta_secs(), 0.));
    }

    // If the user presses W we the entity should jump, and subtract 1 from the jumps_remaining counter.
    // If there are no more jumps remaining the user needs to wait until they touch a MapObject again. This indicates they've landed.
    // If the user is holding W the entitiy should automaticly jump once on the ground.
    if keyboard_input.just_pressed(KeyCode::KeyW) && local_player.jumps_remaining != 0
        || keyboard_input.pressed(KeyCode::KeyW) && local_player.jumps_remaining == 2
    {
        commands.entity(entity).insert(Velocity {
            linvel: vec2(0., 500.),
            angvel: 0.5,
        });

        local_player.jumps_remaining -= 1;
    }
}

/// Handles the local player's attack
pub fn local_player_attack(
    commands: Commands<'_, '_>,
    collision_groups: Res<'_, CollisionGroupSet>,
    app_ctx: ResMut<'_, ApplicationCtx>,
    entity: Entity,
    local_player: &mut Mut<'_, LocalPlayer>,
    transform: &Transform,
) {
    let (attack_collider_width, attack_collider_height) = (50., 50.);
    let attack_collider = Collider::cuboid(attack_collider_width, attack_collider_height);

    let attack_transform = match local_player.direction {
        Direction::Left => Transform::from_xyz(
            transform.translation.x - attack_collider_width,
            transform.translation.y,
            0.,
        ),
        Direction::Right => Transform::from_xyz(
            transform.translation.x + attack_collider_width,
            transform.translation.y,
            0.,
        ),
        Direction::Up => Transform::from_xyz(
            transform.translation.x,
            transform.translation.y + attack_collider_height,
            0.,
        ),
        Direction::Down => Transform::from_xyz(
            transform.translation.x,
            transform.translation.y - attack_collider_height,
            0.,
        ),
    };

    // Spawn in a cuboid and then caluclate the collisions from that
    spawn_attack(
        commands,
        collision_groups,
        app_ctx,
        entity,
        local_player,
        transform,
        attack_collider,
        attack_transform,
    );
}

pub fn local_player_handle(
    query: (
        Entity,
        Mut<LocalPlayer>,
        Mut<KinematicCharacterController>,
        &Transform,
    ),
    mut commands: Commands,
    keyboard_input: ButtonInput<KeyCode>,
    collision_groups: Res<CollisionGroupSet>,
    app_ctx: ResMut<ApplicationCtx>,
    time: Res<Time>,
) {
    // Unpack the tuple created by the tuple
    let (entity, mut local_player, controller, transform) = query;

    if !local_player.player.has_effect(EffectType::Stunned) {
        // Handle the movement of the LocalPlayer
        local_player_movement(
            &mut commands,
            &keyboard_input,
            &time,
            entity,
            &mut local_player,
            controller,
        );

        // Set the variables for the LocalPlayer
        set_movement_direction_var(&keyboard_input, &mut local_player);

        // For this key, it does not matter if its checked with `just_pressed()` or `pressed()`, so we can avoid double checking by just doing both under one check.
        if keyboard_input.just_pressed(KeyCode::KeyS) {
            commands.entity(entity).insert(Velocity {
                linvel: vec2(0., -500.),
                angvel: 0.5,
            });

            // Update latest direction
            local_player.direction = Direction::Down;
        }
    }
    
    // if the player is attacking, handle the local player's attack
    if keyboard_input.just_pressed(KeyCode::Space) {
        local_player_attack(
            commands,
            collision_groups,
            app_ctx,
            entity,
            &mut local_player,
            transform,
        );
    }

    // Increment effects
    local_player.player.tick_effects(time.delta());
}

#[derive(Component, Clone, Default)]
/// A Player instance contains useful information about a Player entity.
pub struct Player {
    /// Contains the health points of the [`Player`].
    pub health: f32,
    /// The current effects the player has.
    /// When an effect has expired, it will automaticly be removed from this list.
    pub effects: Vec<Effect>,
}

impl Player {
    /// Iterates over all the effects, and checks if they're still valid. The effects are removed if the [`Duration`] given to them expires.
    pub fn tick_effects(&mut self, delta: Duration) {
        self.effects.retain_mut(|effect| {
            // If the effect has a `None` duration, it's infinite.
            if let Some(timer) = &mut effect.duration {
                // Increment the timer.
                timer.tick(delta);

                // Check if the timer has finished already, if yes remove the effect.
                if timer.finished() {
                    return false;
                }
            }

            // If the timer hadnt finished yet keep the effect.
            true
        });
    }

    pub fn has_effect(&self, rhs: EffectType) -> bool {
        self.effects.iter().any(|effect| effect.effect_type == rhs)
    }
}
