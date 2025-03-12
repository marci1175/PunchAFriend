use std::time::Duration;

use bevy::{
    asset::Assets,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        event::EventReader,
        query::With,
        system::{Commands, Query, Res, ResMut},
        world::Mut,
    },
    math::vec2,
    render::mesh::Mesh,
    sprite::ColorMaterial,
    time::Time,
    transform::components::Transform,
    winit::{UpdateMode, WinitSettings},
};
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_rapier2d::prelude::{ActiveEvents, Collider, KinematicCharacterController, Velocity};
use bevy_tokio_tasks::TokioTasksRuntime;
use punchafriend::{
    game::{
        collision::CollisionGroupSet,
        combat::{AttackObject, AttackType, Combo},
        pawns::{handle_game_input, Player},
        RandomEngine,
    }, networking::ServerTickUpdate, server::ApplicationCtx, Direction, GameInput, MapElement
};

pub fn setup_game(
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: &CollisionGroupSet,
) {
    // Setup graphics
    commands.spawn(Camera2d);

    commands
        .spawn(Collider::cuboid(500.0, 10.0))
        .insert(Transform::from_xyz(0.0, -200.0, 0.0))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(collision_groups.map_object)
        .insert(MapElement);
}

pub fn tick(
    mut commands: Commands,
    mut app_ctx: ResMut<ApplicationCtx>,
    mut players_query: Query<(
        Entity,
        Mut<Player>,
        Mut<KinematicCharacterController>,
        &Transform,
    )>,
    mut framerate: ResMut<FramepaceSettings>,
    mut rand: ResMut<RandomEngine>,
    runtime: Res<TokioTasksRuntime>,
    collision_groups: Res<CollisionGroupSet>,
    time: Res<Time>,
) {
    // Add tick limiter
    framerate.limiter = Limiter::from_framerate(120.);

    // Increment global tick counter
    let current_tick_count = app_ctx.tick_count.wrapping_add(1);

    // Set the global tick count
    app_ctx.tick_count = current_tick_count;

    if let Some(server_instance) = &mut app_ctx.server_instance {
        if let Some(remote_receiver) = &mut server_instance.server_receiver {
            let connected_clients_clone = server_instance.connected_client_game_sockets.clone();

            while let Ok((client_req, address)) = remote_receiver.try_recv() {
                for mut query_item in players_query.iter_mut() {
                    // If the current player we are iterating on doesn't match the id provided by the client request countinue the iteration.
                    if query_item.1.id != query_item.1.id {
                        continue;
                    }

                    for action in &client_req.inputs {
                        handle_game_input(
                            &mut query_item,
                            &mut commands,
                            *action,
                            &collision_groups,
                            &mut rand.inner,
                            &time,
                        );

                        if *action == GameInput::Exit {
                            let mut entity_commands = commands.entity(query_item.0);
                            
                            entity_commands.despawn();

                            connected_clients_clone.remove(&address);

                            break;
                        }
                    }
                }
            }
        }

        for (_entity, player, _, transform) in players_query.iter() {
            let server_tick_update =
                ServerTickUpdate::new(*transform, player.clone(), current_tick_count);

            let message_bytes = rmp_serde::to_vec(&server_tick_update).unwrap();

            let message_length_bytes = (message_bytes.len() as u32).to_be_bytes();

            for client in server_instance.connected_client_game_sockets.iter() {
                let addr = *client.key();

                let udp_socket = server_instance.udp_socket.clone();
                let message_bytes = message_bytes.clone();

                let mut message_length_bytes = message_length_bytes.to_vec();

                runtime.spawn_background_task(move |_ctx| async move {
                    message_length_bytes.extend(message_bytes);

                    udp_socket
                        .send_to(&message_length_bytes, addr)
                        .await
                        .unwrap();
                });
            }
        }
    }
}

pub fn reset_jump_remaining_for_player(
    collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    character_entity_query: Query<Entity, With<Player>>,
    mut local_player_query: Query<&mut Player>,
) {
    if let Some(colliding_entity) = check_for_collision_with_map_and_player(
        collision_events,
        map_element_query,
        character_entity_query,
    ) {
        if let Ok(mut local_player) = local_player_query.get_mut(colliding_entity) {
            local_player.jumps_remaining = 2;
        }
    }
}

pub fn check_for_collision_with_map_and_player(
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    player_entity_query: Query<Entity, With<Player>>,
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
    mut foreign_character_query: Query<(Entity, &mut Player, &Transform, &Velocity)>,
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

pub fn setup_window(mut winit_settings: ResMut<WinitSettings>) {
    winit_settings.unfocused_mode = UpdateMode::Continuous;
}
