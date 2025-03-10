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
};
use bevy_egui::{
    egui::{self, Align2, Color32, Layout, RichText},
    EguiContexts,
};
use bevy_rapier2d::prelude::{
    ActiveEvents, AdditionalMassProperties, Ccd, Collider, LockedAxes, RigidBody, Velocity,
};
use bevy_tokio_tasks::TokioTasksRuntime;
use egui_toast::{Toast, ToastOptions};

use punchafriend::{
    client::ApplicationCtx,
    game::{collision::CollisionGroupSet, pawns::Player},
    networking::client::ClientConnection,
    MapElement, UiMode,
};
use tokio_util::sync::CancellationToken;

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
) {
    // Get context
    let ctx = context.ctx_mut();

    // Show toasts
    app_ctx.egui_toasts.show(ctx);

    match app_ctx.ui_mode {
        UiMode::Game => {
            // Send the inputs to the sender thread
            if let Some(client_connection) = &app_ctx.client_connection {
                if keyboard_input.just_pressed(KeyCode::Space) {
                    if let Err(err) = client_connection
                        .sender_thread_handle
                        .try_send(punchafriend::GameInput::Jump)
                    {
                        app_ctx.egui_toasts.add(
                            Toast::new()
                                .kind(egui_toast::ToastKind::Error)
                                .text(format!(
                                    "Sending to endpoint handler thread failed: {}",
                                    err.to_string()
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

            // Check for pause key
            if keyboard_input.just_pressed(KeyCode::Escape) {
                app_ctx.ui_mode = UiMode::PauseWindow;
            }
        }
        UiMode::MainMenu => {
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
                        ui.add(egui::Button::new(RichText::from("Options").size(25.)).frame(false));

                        if ui
                            .add(
                                egui::Button::new(RichText::from("Play").size(40.))
                                    .fill(Color32::TRANSPARENT),
                            )
                            .clicked()
                        {
                            // Set ui state
                            app_ctx.ui_mode = UiMode::GameMenu;
                        };

                        ui.add_space(50.);
                    });
                });
        }
        UiMode::GameMenu => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                    if ui.button("Back").clicked() {
                        app_ctx.ui_mode = UiMode::MainMenu;
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
        UiMode::PauseWindow => {
            // Paint the pause menu's backgound
            egui::Area::new("pause_window_background".into()).show(ctx, |ui| {
                ui.painter()
                    .rect_filled(ctx.screen_rect(), 0., Color32::from_black_alpha(200));
            });

            // If the player pauses their game whilst in a game we should display the pause menu.
            egui::Window::new("pause_window")
                .title_bar(false)
                .resizable(false)
                .collapsible(false)
                .anchor(Align2::CENTER_CENTER, egui::vec2(0., 0.))
                .fixed_size(ctx.screen_rect().size() / 3.)
                .show(ctx, |ui| {
                    ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                        ui.add(egui::Button::new("Resume").frame(false));
                        ui.add(egui::Button::new("Options").frame(false));

                        if ui
                            .add(egui::Button::new("Quit Server").frame(false))
                            .clicked()
                        {
                            reset_connection_and_ui(&mut app_ctx);
                        }
                    });
                });
        }
    }

    if let Some(client_connection) = &mut app_ctx.client_connection {
        if let Ok(server_tick_update) = client_connection.main_thread_handle.try_recv() {
            // If the tick we have received is older than the newest one we have we drop it.
            if client_connection.last_tick > server_tick_update.tick_count {
                return;
            }

            // Set the new tick count as the latest tick
            client_connection.last_tick = server_tick_update.tick_count;

            if !players.iter_mut().any(|(_e, mut player, mut transfrom)| {
                let player_found = player.id == server_tick_update.player.id;

                if player_found {
                    *player = server_tick_update.player.clone();
                    *transfrom = server_tick_update.transform;
                }

                player_found
            }) {
                commands
                    .spawn(RigidBody::Dynamic)
                    .insert(Collider::ball(20.0))
                    .insert(server_tick_update.transform)
                    .insert(AdditionalMassProperties::Mass(0.1))
                    .insert(ActiveEvents::COLLISION_EVENTS)
                    .insert(LockedAxes::ROTATION_LOCKED)
                    .insert(collision_groups.player)
                    .insert(Ccd::enabled())
                    .insert(Velocity::default())
                    .insert(server_tick_update.player);
            }
        }
    } else {
        // Try receiving the incoming successful connection to the remote address.
        if let Ok(connection) = app_ctx.connection_receiver.try_recv() {
            match connection {
                Ok(client_connection) => {
                    // Set the window to be displaying game
                    app_ctx.ui_mode = UiMode::Game;

                    // Set the client connection variable
                    app_ctx.client_connection = Some(client_connection);

                    // Game setup was handled here, now its at startup. If we want changing maps we want to modify this.
                    // setup_game(commands, meshes, materials, &collision_groups);
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

fn reset_connection_and_ui(app_ctx: &mut ResMut<'_, ApplicationCtx>) {
    app_ctx.cancellation_token.cancel();

    app_ctx.client_connection = None;

    app_ctx.ui_mode = UiMode::MainMenu;

    app_ctx.cancellation_token = CancellationToken::new();
}

pub fn setup_game(
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
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
