use std::time::Duration;

use bevy::{
    ecs::{
        entity::Entity,
        event::EventReader,
        query::With,
        system::{Commands, Query, Resource},
    },
    math::vec2,
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{CollisionGroups, Group, Velocity};

use crate::Direction;

use super::{
    combat::{AttackObject, AttackType, Combo},
    map::MapElement,
    pawns::Pawn,
};

#[repr(u32)]
pub enum CollisionGroup {
    MapObject = 0b0001,
    ForeignCharacter = 0b0100,
    AttackObj = 0b1000,
}

#[derive(Resource, Clone)]
pub struct CollisionGroupSet {
    /// Collides with all
    pub map_object: CollisionGroups,
    /// Collides with everything except SelfCharacter
    pub player: CollisionGroups,
    /// Collides with MapObject & ForeignCharacter, not SelfCharacter
    pub attack_obj: CollisionGroups,
}

impl Default for CollisionGroupSet {
    fn default() -> Self {
        Self::new()
    }
}

impl CollisionGroupSet {
    pub fn new() -> Self {
        Self {
            map_object: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::MapObject as u32),
                Group::from_bits_truncate(0b1111),
            ),
            player: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::ForeignCharacter as u32),
                Group::from_bits_truncate(
                    CollisionGroup::MapObject as u32 | CollisionGroup::AttackObj as u32,
                ),
            ),
            attack_obj: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::AttackObj as u32),
                Group::from_bits_truncate(
                    CollisionGroup::MapObject as u32 | CollisionGroup::ForeignCharacter as u32,
                ),
            ),
        }
    }
}

pub fn check_for_collision_with_map_and_player(
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    player_entity_query: Query<Entity, With<Pawn>>,
) -> Option<Entity> {
    if let Some(collision) = collision_events.read().next() {
        match collision {
            bevy_rapier2d::prelude::CollisionEvent::Started(
                entity,
                entity2,
                _collision_event_flags,
            ) => {
                let entity1_p = player_entity_query.get(*entity);
                let entity1_m = map_element_query.get(*entity);
                let entity2_p = player_entity_query.get(*entity2);
                let entity2_m = map_element_query.get(*entity2);

                // Check if entity1 is the player and entity2 is the map element or if entity2 is the player and entity1 is the map element
                return if entity1_p.is_ok() && entity2_m.is_ok() {
                    Some(entity1_p.unwrap())
                } else if entity2_p.is_ok() && entity1_m.is_ok() {
                    Some(entity2_p.unwrap())
                } else {
                    None
                };
            }
            bevy_rapier2d::prelude::CollisionEvent::Stopped(
                entity,
                entity2,
                _collision_event_flags,
            ) => {
                let entity1_p = player_entity_query.get(*entity);
                let entity1_m = map_element_query.get(*entity);
                let entity2_p = player_entity_query.get(*entity2);
                let entity2_m = map_element_query.get(*entity2);

                // Check if entity1 is the player and entity2 is the map element or if entity2 is the player and entity1 is the map element
                return if entity1_p.is_ok() && entity2_m.is_ok() {
                    Some(entity1_p.unwrap())
                } else if entity2_p.is_ok() && entity1_m.is_ok() {
                    Some(entity2_p.unwrap())
                } else {
                    None
                };
            }
        }
    }

    None
}

pub fn check_for_collision_with_attack_object(
    mut commands: Commands,
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    mut foreign_character_query: Query<(Entity, &mut Pawn, &Transform, &Velocity)>,
    attack_object_query: Query<(Entity, &AttackObject)>,
) {
    for collision in collision_events.read() {
        match collision {
            bevy_rapier2d::prelude::CollisionEvent::Started(
                entity,
                entity1,
                collision_event_flags,
            ) => {
                let attack_obj_query_result = attack_object_query
                    .iter()
                    .find(|(attck_ent, _)| *attck_ent == *entity || *attck_ent == *entity1);

                let foreign_character_query_result = foreign_character_query
                    .iter_mut()
                    .find(|(foreign_character_entity, _, _, _)| {
                        *foreign_character_entity == *entity
                            || *foreign_character_entity == *entity1
                    })
                    .map(|(e, p, t, v)| (e, p.clone(), *t, *v));

                if let (
                    Some((_attack_ent, attack_object)),
                    Some((
                        foreign_entity,
                        _local_player,
                        foreign_char_transform,
                        foreign_char_velocity,
                    )),
                ) = (attack_obj_query_result, foreign_character_query_result)
                {
                    // We should not apply any forces if the attack hit the player who has spawned the original attack.
                    if attack_object.attack_by == foreign_entity {
                        continue;
                    }

                    let mut colliding_entity_commands = commands.entity(foreign_entity);

                    let attacker_origin_pos = attack_object.attack_origin.translation;
                    let foreign_char_pos = foreign_char_transform.translation;

                    // Decide the direction the enemy should go
                    // If the attacker is closer to the platforms center it should push the enemy the opposite way.
                    let push_left = if attacker_origin_pos.x > foreign_char_pos.x {
                        -1.0
                    } else {
                        1.0
                    };

                    let attacker_result = foreign_character_query
                        .iter_mut()
                        .find(|(ent, _, _, _)| *ent == attack_object.attack_by);

                    // Increment the local player's combo counter and reset its timer
                    if let Some((_, mut local_player, _, _)) = attacker_result {
                        if let Some(combo_counter) = &mut local_player.combo_stats {
                            combo_counter.combo_counter += 1;
                            combo_counter.combo_timer.reset();
                        } else {
                            local_player.combo_stats = Some(Combo::new(Duration::from_secs(2)));
                        }
                    }

                    colliding_entity_commands.insert(Velocity {
                        linvel: vec2(
                            foreign_char_velocity.linvel.x + 180. * push_left,
                            foreign_char_velocity.linvel.y
                                + if attack_object.attack_type
                                    == AttackType::Directional(Direction::Up)
                                {
                                    500.
                                } else if attack_object.attack_type
                                    == AttackType::Directional(Direction::Down)
                                {
                                    -500.
                                } else {
                                    0.
                                },
                        ),
                        // Angles are disabled
                        angvel: 0.,
                    });
                };
            }
            bevy_rapier2d::prelude::CollisionEvent::Stopped(
                entity,
                entity1,
                collision_event_flags,
            ) => {}
        };
    }

    //Remove all the attacks objects after checking for collision
    for (ent, _) in attack_object_query.iter() {
        commands.entity(ent).despawn();
    }
}
