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
        collision::{check_for_collision_with_map_and_player, CollisionGroupSet},
        pawns::{handle_game_input, Player},
        RandomEngine,
    },
    networking::{server::notify_client_about_player_disconnect, ServerTickUpdate},
    server::ApplicationCtx,
    GameInput, MapElement,
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

    if let Some(server_instance) = &mut app_ctx.server_instance {
        if let Some(remote_receiver) = &mut server_instance.server_receiver {
            let connected_clients_clone = server_instance.connected_client_game_sockets.clone();

            while let Ok((client_req, address)) = remote_receiver.try_recv() {
                'query_loop: for mut query_item in players_query.iter_mut() {
                    // If the current player we are iterating on doesn't match the id provided by the client request countinue the iteration.
                    if query_item.1.id != client_req.id {
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

                        if matches!(*action, GameInput::Exit) {
                            println!("Exited");

                            let mut entity_commands = commands.entity(query_item.0);

                            entity_commands.despawn();

                            let connected_clients_clone = connected_clients_clone.clone();

                            let removed_uuid =
                                connected_clients_clone.remove(&address).unwrap().1 .0;

                            runtime.spawn_background_task(move |_ctx| async move {
                                for mut connected_client in connected_clients_clone.iter_mut() {
                                    let (_, tcp_stream) = connected_client.value_mut();

                                    notify_client_about_player_disconnect(tcp_stream, removed_uuid)
                                        .await
                                        .unwrap();
                                }
                            });

                            break 'query_loop;
                        }
                    }
                }
            }
        }

        for (_entity, player, _, position, velocity) in players_query.iter() {
            let server_tick_update =
                ServerTickUpdate::new(*position, *velocity, player.clone(), current_tick_count);

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

pub fn setup_window(
    mut winit_settings: ResMut<WinitSettings>,
    mut framerate: ResMut<FramepaceSettings>,
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
) {
    winit_settings.unfocused_mode = UpdateMode::Continuous;

    framerate.limiter = Limiter::from_framerate(120.);
}
