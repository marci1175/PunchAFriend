use crate::{game::collision::CollisionGroupSet, Direction, GameInput};
use bevy::{
    ecs::{component::Component, entity::Entity, system::Commands, world::Mut},
    math::vec2,
    time::Time,
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{Collider, KinematicCharacterController, Velocity};
use rand::rngs::SmallRng;
use std::time::Duration;
use uuid::Uuid;

use super::combat::{spawn_attack, Combo, Effect, EffectType};

/// This function modifies the direction variable of the `LocalPlayer`, the variable is always the key last pressed by the user.
pub fn set_movement_direction_var(game_input: &GameInput, local_player: &mut Mut<'_, Player>) {
    if *game_input == GameInput::MoveRight {
        // Update latest direction
        local_player.direction = Direction::Right;
    }

    if *game_input == GameInput::MoveLeft {
        // Update latest direction
        local_player.direction = Direction::Left;
    }

    if *game_input == GameInput::MoveJump {
        // Update latest direction
        local_player.direction = Direction::Up;
    }
}

/// Handles the local player's input and modifying the controller of the Entity according to the input given.
pub fn player_movement(
    commands: &mut Commands<'_, '_>,
    game_input: &GameInput,
    time: &Time,
    entity: Entity,
    player: &mut Mut<'_, Player>,
    controller: &mut KinematicCharacterController,
) {
    let move_factor = 450. * {
        if player.has_effect(EffectType::Slowdown) {
            0.5
        } else {
            1.
        }
    };

    if *game_input == GameInput::MoveLeft {
        // Move the local player to the left
        controller.translation = Some(vec2(-move_factor * time.delta_secs(), 0.));
    }

    if *game_input == GameInput::MoveRight {
        // Move the local player to the right
        controller.translation = Some(vec2(move_factor * time.delta_secs(), 0.));
    }

    // If the user presses W we the entity should jump, and subtract 1 from the jumps_remaining counter.
    // If there are no more jumps remaining the user needs to wait until they touch a MapObject again. This indicates they've landed.
    if *game_input == GameInput::MoveJump && dbg!(player.jumps_remaining) != 0 {
        commands.entity(entity).insert(Velocity {
            linvel: vec2(0., 500.),
            angvel: 0.5,
        });

        player.jumps_remaining -= 1;
    }
}

/// Handles the local player's attack
pub fn player_attack(
    commands: &mut Commands,
    collision_groups: &CollisionGroupSet,
    rand: &mut SmallRng,
    entity: Entity,
    local_player: &mut Player,
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
        rand,
        entity,
        local_player,
        transform,
        attack_collider,
        attack_transform,
    );
}

pub fn handle_game_input(
    query: &mut (
        Entity,
        Mut<Player>,
        Mut<KinematicCharacterController>,
        &Transform,
    ),
    commands: &mut Commands,
    game_input: GameInput,
    collision_groups: &CollisionGroupSet,
    rand: &mut SmallRng,
    time: &Time,
) {
    // Unpack the tuple created by the tuple
    let (entity, ref mut player, controller, transform) = query;

    if !player.has_effect(EffectType::Stunned) {
        // Handle the movement of the LocalPlayer
        player_movement(commands, &game_input, time, *entity, player, controller);

        // Set the variables for the LocalPlayer
        set_movement_direction_var(&game_input, player);

        if game_input == GameInput::MoveDuck {
            commands.entity(*entity).insert(Velocity {
                linvel: vec2(0., -500.),
                angvel: 0.5,
            });

            // Update latest direction
            player.direction = Direction::Down;
        }
    }

    // if the player is attacking, handle the local player's attack
    if game_input == GameInput::Attack {
        player_attack(commands, collision_groups, rand, *entity, player, transform);
    }

    // Increment effects
    player.tick_effects(time.delta());
}

#[derive(Component, Clone, Default, serde::Deserialize, serde::Serialize, Debug)]
/// A Player instance contains useful information about a Player entity.
pub struct Player {
    /// Contains the health points of the [`Player`].
    pub health: f32,
    /// The current effects the player has.
    /// When an effect has expired, it will automaticly be removed from this list.
    pub effects: Vec<Effect>,

    pub name: String,

    pub jumps_remaining: u8,

    pub direction: Direction,

    pub combo_stats: Option<Combo>,

    pub id: Uuid,
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

    pub fn new_from_id(id: Uuid) -> Self {
        Self {
            id,
            ..Default::default()
        }
    }
}
