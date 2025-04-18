pub const MINUTE_SECS: u64 = 60;

use chrono::{Local, TimeDelta};
use punchafriend::{
    game::map::{load_map_from_mapinstance, MapObjectUpdate, MovementState},
    networking::{
        server::{send_request_to_all_clients, ServerInstance},
        OngoingGameData, PawnUpdate,
        ServerGameState::{self, Intermission},
        ServerRequest,
    },
};
use std::{f32::consts::PI, sync::Arc, time::Duration};

use bevy::{
    asset::Assets,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        event::EventReader,
        query::{Changed, With, Without},
        system::{Commands, Query, Res, ResMut},
        world::Mut,
    },
    math::Vec3,
    render::mesh::Mesh,
    sprite::ColorMaterial,
    time::{Real, Time, Timer},
    transform::components::Transform,
    winit::{UpdateMode, WinitSettings},
};
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_rapier2d::prelude::{KinematicCharacterController, Velocity};
use bevy_tokio_tasks::TokioTasksRuntime;
use parking_lot::Mutex;
use punchafriend::{
    game::{
        collision::{check_for_collision_with_map_and_player, CollisionGroupSet},
        map::MapElement,
        pawns::{handle_game_input, Pawn},
    },
    networking::{
        server::{notify_client_about_player_disconnect, send_request_to_client},
        GameInput, RemoteServerRequest, ServerTickUpdate,
    },
    server::ApplicationCtx,
    RandomEngine,
};
use tokio::net::tcp::OwnedWriteHalf;

use crate::ui::{
    create_intermission_data_all, notify_valid_clients_intermission,
    notify_valid_clients_map_change,
};

pub fn recv_tick(
    mut commands: Commands,
    mut app_ctx: ResMut<ApplicationCtx>,
    mut players_query: Query<(
        Entity,
        Mut<Pawn>,
        Mut<KinematicCharacterController>,
        &Transform,
        &Velocity,
    )>,
    mut rand: ResMut<RandomEngine>,
    runtime: ResMut<TokioTasksRuntime>,
    collision_groups: Res<CollisionGroupSet>,
    game_time: Res<Time>,
) {
    // Increment global tick counter
    let current_tick_count = app_ctx.tick_count.wrapping_add(1);

    // Set the global tick count
    app_ctx.tick_count = current_tick_count;

    // Handle an existing connection
    if let Some(server_instance) = &mut app_ctx.server_instance {
        if let Some(remote_receiver) = &mut server_instance.client_udp_receiver {
            // Clone the connected clients list's handle
            let connected_clients_clone = server_instance.connected_client_tcp_handles.clone();

            // Iter over all the packets from the clients
            while let Ok((client_req, address)) = remote_receiver.try_recv() {
                // Iter over all the clients so we know which one has sent it
                'query_loop: for mut query_item in players_query.iter_mut() {
                    // If the current player we are iterating on doesn't match the id provided by the client request countinue the iteration.
                    if query_item.1.uuid != client_req.id {
                        continue;
                    }

                    // Iter over all the inputs from the packet
                    for action in &client_req.inputs {
                        // Handle game input
                        handle_game_input(
                            &mut query_item,
                            &mut commands,
                            *action,
                            &collision_groups,
                            &mut rand.inner,
                            &game_time,
                        );

                        // If the client requested to disconnect we should broadcast the message to all of the clients
                        if matches!(*action, GameInput::Exit) {
                            // Get the commands to the disconnected client's entity.
                            let mut entity_commands = commands.entity(query_item.0);

                            // Despawn the disconnected client's entity on the server side.
                            entity_commands.despawn();

                            // Move the DashMap's handle
                            let connected_clients_clone = connected_clients_clone.clone();

                            // The uuid of the client who has disconnected
                            let removed_uuid =
                                connected_clients_clone.remove(&address).unwrap().1 .0;

                            // Spawn an async task to broadcast the disconnection message to the clients
                            notify_players_player_disconnect(
                                &runtime,
                                connected_clients_clone,
                                removed_uuid,
                            );

                            // If we have found the client this message belonged to we can break out of the loop
                            break 'query_loop;
                        }
                    }
                }
            }
        }
    }
}

fn notify_players_game_start(
    runtime: &ResMut<'_, TokioTasksRuntime>,
    connected_client_list: Arc<
        dashmap::DashMap<
            std::net::SocketAddr,
            (
                uuid::Uuid,
                Arc<parking_lot::lock_api::Mutex<parking_lot::RawMutex, OwnedWriteHalf>>,
            ),
        >,
    >,
    map_instance: punchafriend::game::map::MapInstance,
    server_instance: &ServerInstance,
) {
    let round_end_date = Local::now()
        .to_utc()
        .checked_add_signed(TimeDelta::from_std(Duration::from_secs(8 * MINUTE_SECS)).unwrap())
        .unwrap();

    *server_instance.game_state.write() = ServerGameState::OngoingGame(OngoingGameData {
        current_map: map_instance.clone(),
        round_end_date,
    });

    runtime.spawn_background_task(async move |_task| {
        // Iter over all the clients
        for mut entry in connected_client_list.iter_mut() {
            let (_, write_half) = entry.value_mut();

            // Send the message to the client
            send_request_to_client(
                &mut write_half.lock(),
                RemoteServerRequest {
                    request: punchafriend::networking::ServerRequest::ServerGameStateControl(
                        punchafriend::networking::ServerGameState::OngoingGame(
                            OngoingGameData::new(map_instance.clone(), round_end_date),
                        ),
                    ),
                },
            )
            .await
            .unwrap();
        }
    });
}

fn notify_players_player_disconnect(
    runtime: &ResMut<'_, TokioTasksRuntime>,
    connected_clients_clone: std::sync::Arc<
        dashmap::DashMap<std::net::SocketAddr, (uuid::Uuid, Arc<Mutex<OwnedWriteHalf>>)>,
    >,
    removed_uuid: uuid::Uuid,
) {
    runtime.spawn_background_task(move |_ctx| async move {
        // Get the connected clients list
        for connected_client in connected_clients_clone.iter_mut() {
            // Get the handle of the TcpStream established when the client was connecting to the server
            let (_, tcp_stream) = connected_client.value();

            // Send the disconnection message on the TcpStream specified
            notify_client_about_player_disconnect(&mut tcp_stream.lock(), removed_uuid)
                .await
                .unwrap();
        }
    });
}

pub fn send_tick(
    mut app_ctx: ResMut<ApplicationCtx>,
    players_query: Query<
        (
            Entity,
            Mut<Pawn>,
            Mut<KinematicCharacterController>,
            &Transform,
            &Velocity,
        ),
        Changed<Transform>,
    >,
    runtime: Res<TokioTasksRuntime>,
) {
    // Increment global tick counter
    let current_tick_count = app_ctx.tick_count.wrapping_add(1);

    // Set the global tick count
    app_ctx.tick_count = current_tick_count;

    if let Some(server_instance) = &mut app_ctx.server_instance {
        // The tick function is only called if an entity changes its position, so we dont need to check for any kind of input from the clients
        // Iter over all the entities
        for (_entity, player, _, position, velocity) in players_query.iter() {
            // Create a ServerTickUpdate from the data provided by the query
            let server_tick_update =
                ServerTickUpdate::new(punchafriend::networking::TickUpdateType::Pawn(
                    PawnUpdate::new(*position, *velocity, player.clone(), current_tick_count),
                ));

            // Serialize the packet into bytes so it can be sent later
            let message_bytes = rmp_serde::to_vec(&server_tick_update).unwrap();

            // Get the lenght of the message and turn it into bytes
            let message_length_bytes = (message_bytes.len() as u32).to_be_bytes();

            // Iter over all of the connected clients
            for client in server_instance.connected_client_tcp_handles.iter() {
                // Fetch client socket address
                let addr = *client.key();

                // Clone the UdpSocket's handle
                let udp_socket = server_instance.udp_socket.clone();

                // Clone the messages' bytes
                let message_bytes = message_bytes.clone();

                // Turn the message length into bytes
                let mut message_length_bytes = message_length_bytes.to_vec();

                // Spawn an async task to send the information to all of the other clients
                runtime.spawn_background_task(move |_ctx| async move {
                    // Create the messaage buffer which is going to be sent
                    message_length_bytes.extend(message_bytes);

                    // Send the message to the client
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
    character_entity_query: Query<Entity, With<Pawn>>,
    mut local_player_query: Query<&mut Pawn>,
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

pub fn setup_window(
    mut winit_settings: ResMut<WinitSettings>,
    mut framerate: ResMut<FramepaceSettings>,
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
) {
    winit_settings.unfocused_mode = UpdateMode::Continuous;

    commands.spawn(Camera2d);

    framerate.limiter = Limiter::from_framerate(120.);
}

pub fn tick(
    mut map_element_query: Query<(Entity, &mut MapElement, &mut Transform)>,
    game_time: Res<Time>,
    runtime: Res<TokioTasksRuntime>,
    app_ctx: Res<ApplicationCtx>,
) {
    if let Some(server_instance) = &app_ctx.server_instance {
        let connected_clients = server_instance.connected_client_tcp_handles.clone();
        let udp_socket = server_instance.udp_socket.clone();

        for (_element, mut map_element, mut transform) in map_element_query.iter_mut() {
            let map_element_init_pos = map_element.initial_position;

            match &mut map_element.object_type {
                // If the map element is static we dont need to send the updated coordinates to the client
                punchafriend::game::map::ObjectType::Static => (),
                punchafriend::game::map::ObjectType::Variable(variable_object) => {
                    match &mut variable_object.movement_type {
                        punchafriend::game::map::ObjectMovement::Circular(
                            object_movement_type,
                            movement_params,
                        ) => {
                            let delta_ang_per_sec = 360.0_f32.to_radians()
                                / movement_params.duration.as_secs_f32()
                                * game_time.delta_secs();

                            movement_params.angle += delta_ang_per_sec;

                            if movement_params.angle > PI * 2.0 {
                                movement_params.angle -= PI * 2.0;
                            }

                            let x = movement_params.center_pos.x
                                + movement_params.radius * movement_params.angle.cos();
                            let y = movement_params.center_pos.y
                                + movement_params.radius * movement_params.angle.sin();

                            transform.translation = Vec3::new(x, y, 0.0);

                            notify_valid_clients_map_change(
                                udp_socket.clone(),
                                &runtime,
                                connected_clients.clone(),
                                MapObjectUpdate {
                                    transform: *transform,
                                    id: map_element.id,
                                },
                            );
                        }
                        punchafriend::game::map::ObjectMovement::Linear(
                            object_movement_type,
                            movement_params,
                        ) => {
                            let object_params = movement_params.clone();

                            match map_element_init_pos {
                                Some(map_element_init_pos) => {
                                    let total_path_length =
                                        object_params.destination_pos - map_element_init_pos;

                                    let sec_step =
                                        total_path_length / object_params.duration.as_secs_f32();

                                    let current_step = sec_step * game_time.delta_secs();

                                    match variable_object.movement_state.clone() {
                                        punchafriend::game::map::MovementState::In => {
                                            transform.translation +=
                                                Vec3::new(current_step.x, current_step.y, 0.);

                                            if transform.translation.x.abs()
                                                > object_params.destination_pos.x
                                                && transform.translation.y
                                                    <= object_params.destination_pos.y
                                            {
                                                variable_object.movement_state = MovementState::Out;
                                            }
                                        }
                                        punchafriend::game::map::MovementState::Out => {
                                            transform.translation -=
                                                Vec3::new(current_step.x, current_step.y, 0.);

                                            if transform.translation.x.abs()
                                                > object_params.destination_pos.x
                                                && transform.translation.y
                                                    >= object_params.destination_pos.y
                                            {
                                                variable_object.movement_state = MovementState::In;
                                            }
                                        }
                                    }

                                    notify_valid_clients_map_change(
                                        udp_socket.clone(),
                                        &runtime,
                                        connected_clients.clone(),
                                        MapObjectUpdate {
                                            transform: *transform,
                                            id: map_element.id,
                                        },
                                    );
                                }
                                None => {
                                    eprintln!("A `Variable` map element has been created, but the `initial_position` was never set.")
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn frame(
    mut app_ctx: ResMut<ApplicationCtx>,
    real_time: Res<Time<Real>>,
    winit_settings: ResMut<WinitSettings>,
    framerate: ResMut<FramepaceSettings>,
    mut commands: Commands,
    current_game_objects: Query<(Entity, &MapElement, &mut Transform), Without<Pawn>>,
    runtime: ResMut<TokioTasksRuntime>,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    mut players_query: Query<
        (
            Entity,
            Mut<Pawn>,
            Mut<KinematicCharacterController>,
            &Transform,
            &Velocity,
        ),
        Without<MapElement>,
    >,
    collision_groups: Res<CollisionGroupSet>,
) {
    // Increment the round timer, to know when does this round finish
    if let Some(round_timer) = &mut app_ctx.game_round_timer {
        round_timer.tick(real_time.delta());
    }

    // If there is any existing intermission timer increment it
    if let Some(intermission_timer) = &mut app_ctx.intermission_timer {
        intermission_timer.tick(real_time.delta());
    }

    // If there is a round timer check the state of it
    if let Some(round_timer) = app_ctx.game_round_timer.clone() {
        if round_timer.finished() {
            if let Some(instance) = &mut app_ctx.server_instance {
                let client_list = instance.connected_client_tcp_handles.clone();

                let intermission_data = create_intermission_data_all();

                *instance.game_state.write() =
                    ServerGameState::Intermission(intermission_data.clone());

                notify_valid_clients_intermission(&runtime, client_list, intermission_data);

                app_ctx.game_round_timer = None;
                app_ctx.intermission_timer =
                    Some(Timer::from_seconds(30., bevy::time::TimerMode::Once));
            }
        }
    }

    // If there is any existing intermission timer get the immutable state of it
    if let Some(timer) = app_ctx.intermission_timer.clone() {
        if let Some(server_instance) = &app_ctx.server_instance {
            // If the countdown has ended or all of the votes have been casted notify all the clients about the intermission end, and send the new map.
            if timer.finished()
                || (app_ctx.intermission_total_votes
                    == server_instance.connected_client_tcp_handles.len())
                    && !server_instance.connected_client_tcp_handles.is_empty()
            {
                let game_state = server_instance.game_state.read().clone();

                if let Intermission(intermission_data) = game_state.clone() {
                    let most_voted_entry =
                        intermission_data.selectable_maps.iter().max_by_key(|e| e.1);

                    if let Some((voted_map_name, _vote_count)) = most_voted_entry {
                        let connected_client_list =
                            server_instance.connected_client_tcp_handles.clone();

                        let map_instance = voted_map_name.into_map_instance();

                        let map_instance_clone = map_instance.clone();

                        notify_players_game_start(
                            &runtime,
                            connected_client_list,
                            map_instance,
                            server_instance,
                        );

                        load_map_from_mapinstance(
                            map_instance_clone.clone(),
                            &mut commands,
                            collision_groups.clone(),
                            current_game_objects,
                        );

                        *(server_instance.game_state.write()) =
                            ServerGameState::OngoingGame(OngoingGameData {
                                current_map: map_instance_clone.clone(),
                                round_end_date: Local::now()
                                    .to_utc()
                                    .checked_add_signed(TimeDelta::seconds(60 * 8))
                                    .unwrap(),
                            });
                    }
                }

                // Reset the timer's state
                app_ctx.intermission_timer = None;

                // Reset the round timer's state
                app_ctx.game_round_timer = Some(Timer::new(
                    Duration::from_secs(60 * 8),
                    bevy::time::TimerMode::Once,
                ));

                app_ctx.intermission_total_votes = 0;
            }
        }

        if let Some(server_instance) = &mut app_ctx.server_instance {
                                        let connected_clients_clone = server_instance.connected_client_tcp_handles.clone();
                                        // If there is a tcp_listener try receiving the messages sent by the sender thread
            if let Some(tcp_receiver) = &mut server_instance.client_tcp_receiver {
                // Try receiving the message
                if let Ok((message, socket_addr)) = tcp_receiver.try_recv() {
                    //  Match the message type
                    match message.request {
                        punchafriend::networking::ClientRequest::Vote(
                            voted_map_name_discriminant,
                        ) => {
                            // If the client has sent a message check the state of the server.
                            match &mut *server_instance.game_state.clone().write() {
                                punchafriend::networking::ServerGameState::Pause => {}
                                punchafriend::networking::ServerGameState::Intermission(
                                    server_intermission_data,
                                ) => {
                                    if let Some(idx) = server_intermission_data
                                        .selectable_maps
                                        .iter()
                                        .position(|(map, _)| *map == voted_map_name_discriminant)
                                    {
                                        // Increment the voted map's vote count
                                        server_intermission_data.selectable_maps[idx].1 += 1;

                                        // Increment total round count, to check if all the clients have voted
                                        app_ctx.intermission_total_votes += 1;
                                        
                                        runtime.spawn_background_task(async move |_ctx| {
                                            send_request_to_all_clients(RemoteServerRequest { request: ServerRequest::PlayerVote((message.uuid.clone(), voted_map_name_discriminant)) }, connected_clients_clone).await;
                                        });
                                    }
                                }
                                punchafriend::networking::ServerGameState::OngoingGame(
                                    ongoing_game_data,
                                ) => {
                                    let connected_client_tcp_handles =
                                        server_instance.connected_client_tcp_handles.clone();

                                    let socket_addr = socket_addr;
                                    let ongoing_game_data = ongoing_game_data.clone();

                                    runtime.spawn_background_task(async move |_ctx| {
                                        if let Some(handle) = connected_client_tcp_handles
                                            .get(&socket_addr)
                                        {
                                            let (_, tcp_write) = handle.value();

                                            send_request_to_client(
                                                &mut tcp_write.lock(), 
                                                RemoteServerRequest {
                                                    request: punchafriend::networking::ServerRequest::ServerGameStateControl(
                                                        punchafriend::networking::ServerGameState::OngoingGame(
                                                            OngoingGameData::new(ongoing_game_data.current_map.clone(), ongoing_game_data.round_end_date)
                                                        )
                                                    )
                                                }
                                            ).await.unwrap();
                                        }
                                    });
                                }
                            };
                        }
                        punchafriend::networking::ClientRequest::RTTMeasurement(timestamp) => {
                            let connected_client_tcp_handles =
                                server_instance.connected_client_tcp_handles.clone();

                            runtime.spawn_background_task(async move |_ctx| {
                                if let Some(handle) = connected_client_tcp_handles
                                    .get(&socket_addr)
                                {
                                    let (_, tcp_write) = handle.value();

                                    send_request_to_client(
                                        &mut tcp_write.lock(), 
                                        RemoteServerRequest {
                                            request: punchafriend::networking::ServerRequest::RTTMeasurement(timestamp)
                                        }
                                    ).await.unwrap();
                                }
                            });
                        }
                        punchafriend::networking::ClientRequest::PawnTypeChange(
                            desired_pawn_type,
                        ) => {
                            if let Some((_entity, mut pawn, ..)) = players_query
                                .iter_mut()
                                .find(|(_e, pawn, ..)| pawn.uuid == message.uuid)
                            {
                                pawn.pawn_type = desired_pawn_type;

                                let connected_clients_clone =
                                    server_instance.connected_client_tcp_handles.clone();

                                runtime.spawn_background_task(async move |_ctx| {
                                    send_request_to_all_clients(
                                        RemoteServerRequest {
                                            request: ServerRequest::PawnTypeChange((
                                                message.uuid,
                                                desired_pawn_type,
                                            )),
                                        },
                                        connected_clients_clone,
                                    )
                                    .await;
                                });
                            } else {
                                eprintln!(
                                    "`PawnType` change requested, but client not found at uuid."
                                )
                            }
                        }
                        punchafriend::networking::ClientRequest::ClientPawnSync => {
                            let mut pawn_updates: Vec<PawnUpdate> = vec![];

                            for (_entity, pawn, _controller, transform, velocity) in
                                players_query.iter()
                            {
                                pawn_updates.push(PawnUpdate::new(
                                    *transform,
                                    *velocity,
                                    pawn.clone(),
                                    1,
                                ));
                            }

                            let connected_client_tcp_handles =
                                server_instance.connected_client_tcp_handles.clone();

                            runtime.spawn_background_task(async move |_ctx| {
                                if let Some(handle) = connected_client_tcp_handles
                                    .get(&socket_addr)
                                {
                                    let (_, tcp_write) = handle.value();

                                    send_request_to_client(
                                        &mut tcp_write.lock(), 
                                        RemoteServerRequest {
                                            request: punchafriend::networking::ServerRequest::ClientPawnSync(pawn_updates)
                                        }
                                    ).await.unwrap();
                                }
                            });
                        }
                    }
                }
            }
        }
    }
}
