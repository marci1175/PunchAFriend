use crate::{game::collision::CollisionGroupSet, networking::GameInput, Direction};
use bevy::{
    ecs::{component::Component, entity::Entity, system::Commands, world::Mut},
    math::vec2,
    time::Time,
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{
    ActiveEvents, AdditionalMassProperties, Ccd, CharacterLength, Collider, CollisionGroups,
    Friction, KinematicCharacterController, LockedAxes, RigidBody, Velocity,
};
use rand::rngs::SmallRng;
use std::time::Duration;
use uuid::Uuid;

use super::{
    collision::LastInteractedPawn,
    combat::{spawn_attack, Combo, Effect, EffectType},
};

/// This function modifies the direction variable of the `LocalPlayer`, the variable is always the key last pressed by the user.
pub fn set_movement_direction_var(game_input: &GameInput, local_player: &mut Mut<'_, Pawn>) {
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
    player: &mut Mut<'_, Pawn>,
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
    if *game_input == GameInput::MoveJump && player.jumps_remaining != 0 {
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
    local_player: &mut Pawn,
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
        Mut<Pawn>,
        Mut<KinematicCharacterController>,
        &Transform,
        &Velocity,
    ),
    commands: &mut Commands,
    game_input: GameInput,
    collision_groups: &CollisionGroupSet,
    rand: &mut SmallRng,
    time: &Time,
) {
    // Unpack the tuple created by the tuple
    let (entity, ref mut player, controller, transform, _) = query;

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
pub struct Pawn {
    /// Contains the health points of the [`Player`].
    pub health: f32,
    /// The current effects the player has.
    /// When an effect has expired, it will automaticly be removed from this list.
    pub effects: Vec<Effect>,

    pub jumps_remaining: u8,

    pub direction: Direction,

    pub combo_stats: Option<Combo>,

    pub uuid: Uuid,

    pub pawn_attributes: PawnAttribute,

    pub pawn_type: PawnType,
}

impl Pawn {
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
            uuid: id,
            ..Default::default()
        }
    }
}

#[derive(
    Default,
    Clone,
    Copy,
    serde::Deserialize,
    serde::Serialize,
    Debug,
    strum::EnumCount,
    strum::VariantArray,
    strum::Display,
)]
pub enum PawnType {
    #[default]
    Knight,
    Ninja,
    Soldier,
    Human,
    Schoolgirl,
}

impl PawnType {
    pub fn into_pawn_attribute(&self) -> PawnAttribute {
        match self {
            PawnType::Knight => PawnAttribute {
                speed: 0.8,
                jump_height: 0.8,
                attack_speed: 0.6,
                attack_knockback: 2.,
            },
            PawnType::Ninja => PawnAttribute {
                speed: 1.7,
                jump_height: 2.,
                attack_speed: 1.6,
                attack_knockback: 0.6,
            },
            PawnType::Soldier => PawnAttribute {
                speed: 1.0,
                jump_height: 1.0,
                attack_speed: 1.0,
                attack_knockback: 1.0,
            },
            PawnType::Human => PawnAttribute {
                speed: 1.4,
                jump_height: 1.4,
                attack_speed: 1.0,
                attack_knockback: 0.2,
            },
            PawnType::Schoolgirl => PawnAttribute {
                speed: 1.8,
                jump_height: 1.0,
                attack_speed: 2.0,
                attack_knockback: 0.3,
            },
        }
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug)]
pub struct PawnAttribute {
    pub speed: f32,
    pub jump_height: f32,
    pub attack_speed: f32,
    pub attack_knockback: f32,
}

impl Default for PawnAttribute {
    fn default() -> Self {
        Self {
            speed: 1.,
            jump_height: 1.,
            attack_speed: 1.,
            attack_knockback: 1.,
        }
    }
}

pub trait CustomAttack {
    fn spawn_attack(&self, commands: Commands);
}

pub fn spawn_pawn(commands: &mut Commands, uuid: Uuid, collision_group: CollisionGroups) {
    commands
        .spawn(RigidBody::Dynamic)
        .insert(Collider::cuboid(20.0, 30.0))
        .insert(Transform::from_xyz(0., 100., 0.))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(LockedAxes::ROTATION_LOCKED)
        .insert(AdditionalMassProperties::Mass(1.))
        .insert(Friction::coefficient(1.))
        .insert(KinematicCharacterController {
            apply_impulse_to_dynamic_bodies: false,
            snap_to_ground: Some(CharacterLength::Relative(0.2)),
            ..Default::default()
        })
        .insert(collision_group)
        .insert(Ccd::enabled())
        .insert(Velocity::default())
        .insert(LastInteractedPawn::default())
        .insert(Pawn::new_from_id(uuid));
}
