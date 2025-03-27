use std::sync::Arc;

use bevy::{
    asset::Assets,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        event::EventReader,
        query::{Changed, With},
        system::{Commands, Query, Res, ResMut},
        world::Mut,
    },
    render::mesh::Mesh,
    sprite::ColorMaterial,
    time::Time,
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
        pawns::{handle_game_input, Player},
    },
    networking::{
        server::notify_client_about_player_disconnect,
        GameInput, RemoteClientRequest, ServerTickUpdate,
    },
    server::ApplicationCtx,
    RandomEngine,
};
use tokio::io::AsyncReadExt;

pub fn recv_tick(
    mut commands: Commands,
    mut app_ctx: ResMut<ApplicationCtx>,
    mut players_query: Query<(
        Entity,
        Mut<Player>,
        Mut<KinematicCharacterController>,
        &Transform,
        &Velocity,
    )>,
    mut rand: ResMut<RandomEngine>,
    runtime: Res<TokioTasksRuntime>,
    collision_groups: Res<CollisionGroupSet>,
    time: Res<Time>,
) {
    // Increment global tick counter
    let current_tick_count = app_ctx.tick_count.wrapping_add(1);

    // Set the global tick count
    app_ctx.tick_count = current_tick_count;

    let mut connected_clients_list = app_ctx
        .server_instance
        .as_ref()
        .map(|inner| inner.connected_client_game_sockets.clone());

    // If there is any existing intermission timer increment it
    if let Some(timer) = &mut app_ctx.intermission_timer {
        timer.tick(time.delta());

        if let Some(connected_clients) = &mut connected_clients_list {
            for connected_client in connected_clients.iter_mut() {
                let (_, tcp_stream_lock) = connected_client.value();

                let mut message_length_buf = vec![0; 4];

                let tcp_stream = tcp_stream_lock.lock();
                let tcp_stream_lock_clone = tcp_stream_lock.clone();

                if let Ok(_read_bytes) = tcp_stream.try_read(&mut message_length_buf) {
                    let message_length = u32::from_be_bytes(message_length_buf.try_into().unwrap());

                    runtime.spawn_background_task(move |_task| async move {
                        let mut buf = vec![0; message_length as usize];

                        tcp_stream_lock_clone
                            .lock()
                            .read_exact(&mut buf)
                            .await
                            .unwrap();

                        let client_request =
                            rmp_serde::from_slice::<RemoteClientRequest>(&buf).unwrap();

                        match client_request.request {
                            punchafriend::networking::ClientRequest::Vote(
                                map_name_discriminants,
                            ) => todo!(),
                        }
                    });
                }
            }
        }

        // If the countdown has ended notify all the client
        if timer.finished() {

            // send_request_to_client(tcp_stream, RemoteServerRequest {request: punchafriend::networking::ServerRequest::ServerGameStateControl(punchafriend::networking::ServerGameState::OngoingGame(()))})
        }
    }

    // Handle an existing connection
    if let Some(server_instance) = &mut app_ctx.server_instance {
        if let Some(remote_receiver) = &mut server_instance.server_receiver {
            // Clone the connected clients list's handle
            let connected_clients_clone = server_instance.connected_client_game_sockets.clone();

            // Iter over all the packets from the clients
            while let Ok((client_req, address)) = remote_receiver.try_recv() {
                // Iter over all the clients so we know which one has sent it
                'query_loop: for mut query_item in players_query.iter_mut() {
                    // If the current player we are iterating on doesn't match the id provided by the client request countinue the iteration.
                    if query_item.1.id != client_req.id {
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
                            &time,
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

fn notify_players_player_disconnect(
    runtime: &Res<'_, TokioTasksRuntime>,
    connected_clients_clone: std::sync::Arc<
        dashmap::DashMap<std::net::SocketAddr, (uuid::Uuid, Arc<Mutex<tokio::net::TcpStream>>)>,
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
            Mut<Player>,
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
                ServerTickUpdate::new(*position, *velocity, player.clone(), current_tick_count);

            // Serialize the packet into bytes so it can be sent later
            let message_bytes = rmp_serde::to_vec(&server_tick_update).unwrap();

            // Get the lenght of the message and turn it into bytes
            let message_length_bytes = (message_bytes.len() as u32).to_be_bytes();

            // Iter over all of the connected clients
            for client in server_instance.connected_client_game_sockets.iter() {
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
