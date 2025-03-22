use std::{fs, path::PathBuf, time::Duration};

use bevy::{
    app::AppExit,
    asset::{AssetServer, Assets},
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        event::EventReader,
        system::{Commands, Query, Res, ResMut},
    },
    input::{keyboard::KeyCode, ButtonInput},
    math::UVec2,
    render::mesh::Mesh,
    sprite::{ColorMaterial, Sprite, TextureAtlas, TextureAtlasLayout},
    time::{Time, Timer},
    transform::components::Transform,
    winit::{UpdateMode, WinitSettings},
};
use bevy_framepace::FramepaceSettings;
use bevy_rapier2d::prelude::{
    ActiveEvents, AdditionalMassProperties, Ccd, Collider, LockedAxes, RigidBody, Velocity,
};
use egui_toast::{Toast, ToastOptions};

use miniz_oxide::deflate::CompressionLevel;
use punchafriend::{
    client::ApplicationCtx,
    game::{collision::CollisionGroupSet, pawns::Player},
    networking::GameInput,
    MapElement, PauseWindowState, UiLayer,
};
use tokio_util::sync::CancellationToken;

use crate::app::lib::{AnimationState, UniqueLastTickCount};

pub fn handle_server_output(
    mut app_ctx: ResMut<'_, ApplicationCtx>,
    mut players: Query<
        '_,
        '_,
        (
            Entity,
            &mut Player,
            &mut Transform,
            &mut Velocity,
            &mut UniqueLastTickCount,
            &mut Sprite,
            &mut AnimationState,
        ),
    >,
    mut commands: Commands<'_, '_>,
    collision_groups: Res<'_, CollisionGroupSet>,
    asset_server: Res<AssetServer>,
    time: Res<Time>,
) {
    let layout = app_ctx.texture_atlas_layouts.clone();

    if let Some(client_connection) = &mut app_ctx.client_connection {
        while let Ok(server_tick_update) = client_connection.server_tick_receiver.try_recv() {
            // If the tick we have received is older than the newest one we have we drop it.
            if client_connection.last_tick > server_tick_update.tick_count {
                return;
            }

            if !players.iter_mut().any(
                |(
                    _e,
                    mut player,
                    mut transfrom,
                    mut velocity,
                    mut unique_tick_count,
                    mut sprite,
                    mut animation_state,
                )| {
                    let player_updatable = player.id == server_tick_update.player.id
                        && unique_tick_count.get_inner() <= server_tick_update.tick_count;

                    if player_updatable {
                        // Only modify the animation's state if the player has moved!
                        if transfrom.translation != server_tick_update.position.translation {
                            if let Some(atlas) = &mut sprite.texture_atlas {
                                atlas.index = animation_state.animate_state(time.delta());
                            }
                        }

                        *player = server_tick_update.player.clone();
                        *transfrom = server_tick_update.position;
                        *velocity = server_tick_update.velocity;

                        // Set the new tick count as the latest tick for this entity
                        unique_tick_count.with_tick(server_tick_update.tick_count);
                    }

                    player_updatable
                },
            ) {
                let animation_state = AnimationState::new(
                    Timer::new(
                        Duration::from_secs_f32(0.1),
                        bevy::time::TimerMode::Repeating,
                    ),
                    6,
                    crate::app::lib::AnimationType::Walk,
                );

                let starting_anim_idx = animation_state.animation_idx;

                commands
                    .spawn(RigidBody::Dynamic)
                    .insert(Collider::cuboid(20.0, 30.0))
                    .insert(server_tick_update.position)
                    .insert(AdditionalMassProperties::Mass(0.1))
                    .insert(ActiveEvents::COLLISION_EVENTS)
                    .insert(LockedAxes::ROTATION_LOCKED)
                    .insert(collision_groups.player)
                    .insert(Ccd::enabled())
                    .insert(Velocity::default())
                    .insert(UniqueLastTickCount::new(0))
                    .insert(animation_state)
                    .insert(Sprite::from_atlas_image(
                        asset_server.load("../assets/walk.png"),
                        TextureAtlas {
                            layout,
                            index: starting_anim_idx,
                        },
                    ))
                    .insert(server_tick_update.player);

                break;
            }
        }

        if let Ok(remote_request) = client_connection.remote_receiver.try_recv() {
            let uuid = remote_request.id;

            match remote_request.request {
                punchafriend::networking::ServerRequest::PlayerDisconnect => {
                    // Find the Entity with the designated uuid
                    for (entity, player, _, _, _, _, _) in players.iter() {
                        // Check for the correct uuid
                        if player.id == uuid {
                            // Despawn the entity
                            commands.entity(entity).despawn();

                            // Break out from the loop
                            break;
                        }
                    }
                },
                punchafriend::networking::ServerRequest::GameFlowControl(game_flow_control) => {
                    
                }
            }
        }
    } else {
        // Try receiving the incoming successful connection to the remote address.
        if let Ok(connection) = app_ctx.connection_receiver.try_recv() {
            match connection {
                Ok(client_connection) => {
                    // Iterate over all of the players
                    for (entity, _, _, _, _, _, _) in players.iter() {
                        // Despawn all of the existing players, to clear out players left from a different match
                        commands.entity(entity).despawn();
                    }

                    // Set the window to be displaying game
                    app_ctx.ui_layer = UiLayer::Game;

                    // Set the client connection variable
                    app_ctx.client_connection = Some(client_connection);
                }
                Err(error) => {
                    app_ctx.egui_toasts.add(
                        Toast::new()
                            .kind(egui_toast::ToastKind::Error)
                            .text(format!("Connection Failed: {}", error))
                            .options(
                                ToastOptions::default()
                                    .duration(Some(Duration::from_secs(3)))
                                    .show_progress(true),
                            ),
                    );
                }
            }
        }
    }
}

pub fn handle_user_input(
    mut app_ctx: ResMut<'_, ApplicationCtx>,
    keyboard_input: Res<'_, ButtonInput<KeyCode>>,
) {
    if app_ctx.ui_layer != UiLayer::Game {
        return;
    }

    // Check for pause key
    if keyboard_input.just_pressed(KeyCode::Escape) {
        app_ctx.ui_layer =
            UiLayer::PauseWindow((PauseWindowState::Main, Box::new(app_ctx.ui_layer.clone())));
    }

    // Send the inputs to the sender thread
    if let Some(client_connection) = &app_ctx.client_connection {
        let mut game_inputs: Vec<GameInput> = vec![];

        for pressed in keyboard_input.get_pressed() {
            match pressed {
                KeyCode::KeyD => game_inputs.push(GameInput::MoveRight),
                KeyCode::KeyA => game_inputs.push(GameInput::MoveLeft),
                KeyCode::KeyS => game_inputs.push(GameInput::MoveDuck),
                _ => continue,
            }
        }

        for just_pressed in keyboard_input.get_just_pressed() {
            match just_pressed {
                KeyCode::Space => game_inputs.push(GameInput::Attack),
                KeyCode::KeyW => game_inputs.push(GameInput::MoveJump),
                _ => continue,
            }
        }

        // If we havent inputted anything dont send the server an empty packet
        if game_inputs.is_empty() {
            return;
        }

        if let Err(err) = client_connection.server_input_sender.try_send(game_inputs) {
            app_ctx.egui_toasts.add(
                Toast::new()
                    .kind(egui_toast::ToastKind::Error)
                    .text(format!(
                        "Sending to endpoint handler thread failed: {}",
                        err
                    ))
                    .options(
                        ToastOptions::default()
                            .duration(Some(Duration::from_secs(3)))
                            .show_progress(true),
                    ),
            );

            reset_connection_and_ui(&mut app_ctx);
        }
    }
}

pub fn reset_connection_and_ui(app_ctx: &mut ResMut<'_, ApplicationCtx>) {
    app_ctx.cancellation_token.cancel();

    app_ctx.client_connection = None;

    app_ctx.ui_layer = UiLayer::MainMenu;

    app_ctx.cancellation_token = CancellationToken::new();
}

pub fn setup_game(
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
    mut winit_settings: ResMut<WinitSettings>,
    framerate: ResMut<FramepaceSettings>,
    mut app_ctx: ResMut<'_, ApplicationCtx>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // Setup graphics
    commands.spawn(Camera2d);

    commands
        .spawn(Collider::cuboid(500.0, 10.0))
        .insert(Transform::from_xyz(0.0, -200.0, 0.0))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(collision_groups.map_object)
        .insert(MapElement);

    winit_settings.unfocused_mode = UpdateMode::Continuous;

    // Get the path of the %APPDATA% key.
    #[cfg(target_os = "windows")]
    let mut app_data_path = PathBuf::from(std::env::var("APPDATA").unwrap());

    // Get the path of the opt key.
    #[cfg(target_os = "linux")]
    let mut app_data_path = PathBuf::from(std::env::var("opt").unwrap());

    // Push the application's folder name to the path.
    app_data_path.push("PunchAFriend");

    // Push the file name
    app_data_path.push("temp");

    // Read data and decompress it
    match fs::read(app_data_path) {
        Ok(read_bytes) => {
            // Decompress data
            let decompressed_data = miniz_oxide::inflate::decompress_to_vec(&read_bytes).unwrap();

            // Serialize bytes into struct
            let data: ApplicationCtx =
                rmp_serde::from_slice(&decompressed_data).unwrap_or_default();

            // Set data
            *app_ctx = data;
        }
        Err(_err) => {
            //The save didnt exist
        }
    }

    // Create the texture atlas grid
    app_ctx.texture_atlas_layouts = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
        UVec2::new(50, 64),
        7,
        1,
        Some(UVec2::new(20, 0)),
        None,
    ));
}

pub fn exit_handler(_exit_events: EventReader<AppExit>, ui_state: Res<ApplicationCtx>) {
    // Get the path of the %APPDATA% key.
    #[cfg(target_os = "windows")]
    let mut app_data_path = PathBuf::from(std::env::var("APPDATA").unwrap());

    // Get the path of the opt key.
    #[cfg(target_os = "linux")]
    let mut app_data_path = PathBuf::from(std::env::var("opt").unwrap());

    // Push the application's folder name to the path.
    app_data_path.push("PunchAFriend");

    // Create all of the folders which are needed for the path to exist
    fs::create_dir_all(app_data_path.clone()).unwrap();

    // Push the file name
    app_data_path.push("temp");

    // Serialize data
    let serialized_data = rmp_serde::to_vec(&*ui_state).unwrap();

    // Write data before compressing it
    fs::write(
        app_data_path,
        miniz_oxide::deflate::compress_to_vec(
            &serialized_data,
            CompressionLevel::BestCompression as u8,
        ),
    )
    .unwrap();
}
