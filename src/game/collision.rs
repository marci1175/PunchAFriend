use std::time::Duration;

use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        event::EventReader,
        query::{Changed, With},
        system::{Commands, Query, Res, Resource},
    },
    math::vec2,
    transform::components::Transform,
};
use bevy_rapier2d::prelude::{CollisionGroups, Group, Velocity};
use bevy_tokio_tasks::TokioTasksRuntime;
use uuid::Uuid;

use crate::{
    networking::{server::send_request_to_all_clients, ClientStatistics, RemoteServerRequest},
    server::ApplicationCtx,
    Direction,
};

use super::{
    combat::{AttackObject, AttackType, Combo},
    map::MapElement,
    pawns::{spawn_pawn, Pawn},
};

#[derive(Component, Debug, Clone, Default)]
pub struct LastInteractedPawn(Option<Uuid>);

impl LastInteractedPawn {
    pub fn set_last_pawn(&mut self, uuid: Uuid) {
        self.0 = Some(uuid);
    }

    pub fn get_inner(&self) -> &Option<Uuid> {
        &self.0
    }
}

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
    pub pawn: CollisionGroups,
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
            pawn: CollisionGroups::new(
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
    mut character_query: Query<(
        Entity,
        &mut Pawn,
        &Transform,
        &Velocity,
        &mut LastInteractedPawn,
    )>,
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

                let character_query_result = character_query
                    .iter_mut()
                    .find(|(character_entity, _, _, _, _)| {
                        *character_entity == *entity || *character_entity == *entity1
                    })
                    .map(|(e, p, t, v, lp)| (e, p.clone(), *t, *v, lp));

                let mut attacker_uuid: Option<Uuid> = None;

                if let (
                    Some((_attack_ent, attack_object)),
                    Some((
                        attacked_entity,
                        attacked_pawn,
                        foreign_char_transform,
                        foreign_char_velocity,
                        last_interacted_pawn,
                    )),
                ) = (attack_obj_query_result, &character_query_result)
                {
                    // We should not apply any forces if the attack hit the player who has spawned the original attack.
                    if attack_object.attack_by == *attacked_entity {
                        continue;
                    }

                    let mut colliding_entity_commands = commands.entity(*attacked_entity);

                    let attacker_origin_pos = attack_object.attack_origin.translation;
                    let character_position = foreign_char_transform.translation;

                    // Decide the direction the enemy should go
                    // If the attacker is closer to the platforms center it should push the enemy the opposite way.
                    let push_left = if attacker_origin_pos.x > character_position.x {
                        -1.0
                    } else {
                        1.0
                    };

                    let attacker_result = character_query
                        .iter_mut()
                        .find(|(ent, _, _, _, _)| *ent == attack_object.attack_by);

                    // Increment the local player's combo counter and reset its timer
                    if let Some((_, mut local_player, _, _, _)) = attacker_result {
                        if let Some(combo_counter) = &mut local_player.combo_stats {
                            combo_counter.combo_counter += 1;
                            combo_counter.combo_timer.reset();
                        } else {
                            local_player.combo_stats = Some(Combo::new(Duration::from_secs(2)));
                        }

                        attacker_uuid = Some(local_player.uuid)
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

                let character_query_result = character_query
                    .iter_mut()
                    .find(|(character_entity, _, _, _, _)| {
                        *character_entity == *entity || *character_entity == *entity1
                    })
                    .map(|(e, p, t, v, lp)| (e, p.clone(), *t, *v, lp));

                if let Some((_, _, _, _, mut last_interacted_pawn)) = character_query_result {
                    if let Some(attacker_uuid) = &attacker_uuid {
                        last_interacted_pawn.set_last_pawn(*attacker_uuid);
                    }
                }
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

pub fn check_players_out_of_bounds(
    runtime: Res<TokioTasksRuntime>,
    players: Query<(Entity, &Pawn, &Transform, &LastInteractedPawn), Changed<Transform>>,
    app_ctx: Res<ApplicationCtx>,
    mut commands: Commands,
    collision_groups: Res<CollisionGroupSet>,
) {
    // Check if there is a server running currently
    if let Some(server_instance) = &app_ctx.server_instance {
        // Create a list of all the modified client statistics.
        let mut modified_client_stats: Vec<ClientStatistics> = Vec::new();

        // Iter over the list of players
        for (e, pawn, position, last_interacted_pawn) in players.iter() {
            // Check if the player contained in the query is out of bounds
            if position.translation.y < -400. {
                let mut client_stats_list_handle = server_instance.connected_clients_stats.write();

                let client_stats_list = client_stats_list_handle
                    .iter()
                    .cloned()
                    .collect::<Vec<ClientStatistics>>();

                for mut client in client_stats_list.clone() {
                    // Find the matching uuid
                    if client.uuid == pawn.uuid {
                        // Remove the original entry
                        client_stats_list_handle.remove(&client.clone());

                        // Modify the entry
                        client.deaths += 1;

                        // Re-insert the entry
                        client_stats_list_handle.insert(client.clone());

                        // Store the modified client stats entry in the list so that it can be sent later to the clients
                        modified_client_stats.push(client);

                        // Check who interacted last with the pawn
                        if let Some(last_int_player_uuid) = last_interacted_pawn.get_inner() {
                            for mut client_stats in client_stats_list.clone() {
                                if client_stats.uuid == *last_int_player_uuid {
                                    client_stats_list_handle.remove(&client_stats);

                                    // Increment stats
                                    client_stats.kills += 1;
                                    client_stats.score += 100;

                                    // Update the BTreeSet on the serverside
                                    client_stats_list_handle.insert(client_stats.clone());

                                    // Store the modified client stats entry in the list so that it can be sent later to the clients
                                    modified_client_stats.push(client_stats);
                                }
                            }
                        }

                        // Despawn pawn which has fallen off
                        commands.entity(e).despawn();

                        // Respawn the pawn
                        spawn_pawn(&mut commands, pawn.uuid, collision_groups.pawn);
                    }
                }
            }
        }
        // Clone the list handle
        let connected_clients_clone = server_instance.connected_client_tcp_handles.clone();
        if !modified_client_stats.is_empty() {
            // Create an async task for sending the updates to the clients
            runtime.spawn_background_task(async move |_ctx| {
                // Notify all the clients about the new entries
                send_request_to_all_clients(
                    RemoteServerRequest {
                        request: crate::networking::ServerRequest::PlayersStatisticsChange(
                            modified_client_stats,
                        ),
                    },
                    connected_clients_clone,
                )
                .await;
            });
        }
    }
}
