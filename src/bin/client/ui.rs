use std::time::Duration;

use bevy::{
    asset::Assets,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res, ResMut},
    },
    input::{keyboard::KeyCode, ButtonInput},
    render::mesh::Mesh,
    sprite::ColorMaterial,
    time::Time,
    transform::components::Transform,
    winit::{UpdateMode, WinitSettings},
};
use bevy_egui::{
    egui::{self, Align2, Color32, Layout, Pos2, RichText, Sense, Slider},
    EguiContexts,
};
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_rapier2d::prelude::{
    ActiveEvents, AdditionalMassProperties, Ccd, Collider, LockedAxes, RigidBody, Velocity,
};
use bevy_tokio_tasks::TokioTasksRuntime;
use egui_toast::{Toast, ToastOptions};

use punchafriend::{
    client::ApplicationCtx,
    game::{collision::CollisionGroupSet, pawns::Player},
    networking::client::ClientConnection,
    GameInput, MapElement, PauseWindowState, UiLayer,
};
use tokio_util::sync::CancellationToken;

use crate::lib::UniqueLastTickCount;

#[derive(Debug, Clone, Default)]
pub struct UiState {
    connect_to_address: String,
}

pub fn ui_system(
    mut context: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    runtime: ResMut<TokioTasksRuntime>,
    mut players: Query<(Entity, &mut Player, &mut Transform)>,
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
    mut framepace: ResMut<FramepaceSettings>,
) {
    // Get context
    let ctx = context.ctx_mut();

    // Show toasts
    app_ctx.egui_toasts.show(ctx);

    match app_ctx.ui_layer.clone() {
        UiLayer::Game => {
            // handle_user_input(app_ctx, keyboard_input);
        }
        UiLayer::MainMenu => {
            // Display main title.
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::from("Punch A Friend!").size(50.));
                });
            });

            // Display the main menu options.
            egui::TopBottomPanel::bottom("main_menu_options")
                .show_separator_line(false)
                .show(ctx, |ui| {
                    ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
                        ui.add(egui::Button::new(RichText::from("Mods").size(25.)).frame(false));

                        if ui
                            .add(
                                egui::Button::new(RichText::from("Options").size(25.)).frame(false),
                            )
                            .clicked()
                        {
                            app_ctx.ui_layer = UiLayer::PauseWindow((
                                PauseWindowState::Settings,
                                Box::new(app_ctx.ui_layer.clone()),
                            ));
                        };

                        if ui
                            .add(
                                egui::Button::new(RichText::from("Play").size(40.))
                                    .fill(Color32::TRANSPARENT),
                            )
                            .clicked()
                        {
                            // Set ui state
                            app_ctx.ui_layer = UiLayer::GameMenu;
                        };

                        ui.add_space(50.);
                    });
                });
        }
        UiLayer::GameMenu => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                    if ui.button("Back").clicked() {
                        app_ctx.ui_layer = UiLayer::MainMenu;
                    }

                    ui.label("Connect to a Game Server:");

                    ui.text_edit_singleline(&mut app_ctx.ui_state.connect_to_address);

                    if ui.button("Connect").clicked() && app_ctx.client_connection.is_none() {
                        // Clone the address so it can be moved.
                        let address = app_ctx.ui_state.connect_to_address.clone();

                        // Move the sender
                        let sender = app_ctx.connection_sender.clone();

                        // Set the channel
                        let cancellation_token = app_ctx.cancellation_token.clone();

                        // Create the connecting thread
                        runtime.spawn_background_task(|_ctx| async move {
                            // Attempt to make a connection to the remote address.
                            let client_connection =
                                ClientConnection::connect_to_address(address, cancellation_token)
                                    .await;

                            // Send it to the front end no matter the end result.
                            sender.send(client_connection).await.unwrap();
                        });
                    };
                });
            });
        }
        UiLayer::PauseWindow((inner_state, state_before)) => {
            // Paint the pause menu's backgound
            egui::Area::new("pause_window_background".into()).show(ctx, |ui| {
                ui.painter()
                    .rect_filled(ctx.screen_rect(), 0., Color32::from_black_alpha(200));

                // Consume all interactions
                ui.interact(
                    ctx.screen_rect(),
                    "consume_input".into(),
                    Sense::click_and_drag(),
                );
            });

            let window_state = match inner_state {
                punchafriend::PauseWindowState::Main => {
                    // If the player pauses their game whilst in a game we should display the pause menu.
                    egui::Window::new("pause_window")
                        .title_bar(false)
                        .resizable(false)
                        .collapsible(false)
                        .anchor(Align2::CENTER_CENTER, egui::vec2(0., 0.))
                        .fixed_size(ctx.screen_rect().size() / 3.)
                        .show(ctx, |ui| {
                            ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                                if ui.add(egui::Button::new("Resume").frame(false)).clicked() {
                                    app_ctx.ui_layer = UiLayer::Game;
                                }

                                if ui.add(egui::Button::new("Options").frame(false)).clicked() {
                                    app_ctx.ui_layer = UiLayer::PauseWindow((
                                        PauseWindowState::Settings,
                                        Box::new(app_ctx.ui_layer.clone()),
                                    ));
                                }

                                if ui
                                    .add(egui::Button::new("Quit Server").frame(false))
                                    .clicked()
                                {
                                    reset_connection_and_ui(&mut app_ctx);
                                }
                            });
                        })
                }
                punchafriend::PauseWindowState::Settings => egui::Window::new("Settings")
                    .resizable(false)
                    .collapsible(false)
                    .anchor(Align2::CENTER_CENTER, egui::vec2(0., 0.))
                    .fixed_size(ctx.screen_rect().size() / 2.)
                    .vscroll(true)
                    .show(ctx, |ui| {
                        ui.label(RichText::from("Video").size(20.).strong());

                        ui.horizontal(|ui| {
                            ui.label("Framerate");

                            let fps_slider =
                                ui.add(Slider::new(&mut app_ctx.settings.fps, 30.0..=600.0));

                            if fps_slider.changed() {
                                framepace.limiter = Limiter::from_framerate(app_ctx.settings.fps);
                            }
                        });
                    }),
            };

            let window_pos_rect = window_state.unwrap().response.rect;

            // Create the exit button
            egui::Area::new("exit_button".into())
                .fixed_pos(Pos2::new(
                    window_pos_rect.max.x - 50.,
                    window_pos_rect.min.y - 20.,
                ))
                .show(ctx, |ui| {
                    if ui.button(RichText::from("Back").strong()).clicked() {
                        app_ctx.ui_layer = *state_before.clone();
                    }
                });
        }
    }
}

pub fn handle_server_output(
    mut app_ctx: ResMut<'_, ApplicationCtx>,
    mut players: Query<'_, '_, (Entity, &mut Player, &mut Transform, &mut Velocity, &mut UniqueLastTickCount)>,
    mut commands: Commands<'_, '_>,
    collision_groups: Res<'_, CollisionGroupSet>,
) {
    if let Some(client_connection) = &mut app_ctx.client_connection {
        while let Ok(server_tick_update) = client_connection.server_tick_receiver.try_recv() {
            // If the tick we have received is older than the newest one we have we drop it.
            if client_connection.last_tick > server_tick_update.tick_count {
                return;
            }

            if !players.iter_mut().any(|(_e, mut player, mut transfrom, mut velocity, mut unique_tick_count)| {
                let player_updatable = player.id == server_tick_update.player.id && unique_tick_count.get_inner() <= server_tick_update.tick_count;

                if player_updatable {
                    *player = server_tick_update.player.clone();
                    *transfrom = server_tick_update.position;
                    *velocity = server_tick_update.velocity;

                    // Set the new tick count as the latest tick for this entity
                    unique_tick_count.with_tick(server_tick_update.tick_count);
                }

                player_updatable
            }) {
                commands
                    .spawn(RigidBody::Dynamic)
                    .insert(Collider::ball(20.0))
                    .insert(server_tick_update.position)
                    .insert(AdditionalMassProperties::Mass(0.1))
                    .insert(ActiveEvents::COLLISION_EVENTS)
                    .insert(LockedAxes::ROTATION_LOCKED)
                    .insert(collision_groups.player)
                    .insert(Ccd::enabled())
                    .insert(Velocity::default())
                    .insert(UniqueLastTickCount::new(0))
                    .insert(server_tick_update.player);

                break;
            }
        }

        if let Ok(remote_request) = client_connection.remote_receiver.try_recv() {
            let uuid = remote_request.id;

            match remote_request.request {
                punchafriend::networking::ServerRequest::PlayerDisconnect => {
                    // Find the Entity with the designated uuid
                    for (entity, player, _, _, _) in players.iter() {
                        // Check for the correct uuid
                        if player.id == uuid {
                            // Despawn the entity
                            commands.entity(entity).despawn();

                            // Break out from the loop
                            break;
                        }
                    }
                }
            }
        }
    } else {
        // Try receiving the incoming successful connection to the remote address.
        if let Ok(connection) = app_ctx.connection_receiver.try_recv() {
            match connection {
                Ok(client_connection) => {
                    // Iterate over all of the players
                    for (entity, _, _, _, _) in players.iter() {
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

        if let Err(err) = client_connection
            .server_input_sender
            .try_send(game_inputs)
        {
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

fn reset_connection_and_ui(app_ctx: &mut ResMut<'_, ApplicationCtx>) {
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
    mut framerate: ResMut<FramepaceSettings>,
) {
    // Setup graphics
    commands.spawn(Camera2d);

    commands
        .spawn(Collider::cuboid(500.0, 10.0))
        .insert(Transform::from_xyz(0.0, -200.0, 0.0))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(collision_groups.map_object)
        .insert(MapElement);

    framerate.limiter = Limiter::from_framerate(60.);

    winit_settings.unfocused_mode = UpdateMode::Continuous;
}
