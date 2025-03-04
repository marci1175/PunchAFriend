use std::time::Duration;

use bevy::{
    ecs::{
        entity::Entity,
        system::{Query, Res, ResMut, Resource},
    },
    time::Time,
};
use bevy_egui::{
    egui::{self, Align2, Color32, Layout, RichText},
    EguiContexts,
};
use bevy_tokio_tasks::TokioTasksRuntime;
use egui_toast::{Toast, ToastOptions, Toasts};
use quinn::rustls::pki_types::CertificateDer;
use tokio::sync::mpsc::{channel, Receiver};

use punchafriend::{client::ApplicationCtx, game::pawns::Player, networking::client::ClientConnection, UiMode};

#[derive(Debug, Clone, Default)]
pub struct UiState {
    connect_to_address: String,
}

pub fn ui_system(
    mut context: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    runtime: ResMut<TokioTasksRuntime>,
    mut local_player: Query<(Entity, &mut Player)>,
    time: Res<Time>,
) {
    // Get context
    let ctx = context.ctx_mut();

    // Show toasts
    app_ctx.egui_toasts.show(ctx);

    match app_ctx.ui_mode {
        UiMode::Game => {
            let local_player = local_player.get_single_mut();

            if let Ok((entity, mut local_player)) = local_player {
                egui::Area::new("game_hud".into())
                    .anchor(Align2::RIGHT_BOTTOM, egui::vec2(-200., -50.))
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.set_min_size(egui::vec2(250., 30.));
                        ui.allocate_ui(ui.available_size(), |ui| {
                            let combo_stats = &mut local_player.combo_stats;

                            if let Some(combo_stats) = combo_stats {
                                ui.label(
                                    RichText::from(format!("Combo: {}", combo_stats.combo_counter))
                                        .strong()
                                        .size(20.),
                                );
                                ui.label(
                                    RichText::from(format!(
                                        "Time left: {:.2}s",
                                        (combo_stats.combo_timer.duration().as_secs_f32()
                                            - combo_stats.combo_timer.elapsed_secs())
                                    ))
                                    .strong()
                                    .size(20.),
                                );

                                combo_stats.combo_timer.tick(time.delta());
                            }

                            if let Some(combo) = combo_stats.clone() {
                                if combo.combo_timer.finished() {
                                    *combo_stats = None;
                                }
                            }
                        });
                    });
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

                        // Create a new channel pair
                        let (sender, receiver) = channel::<anyhow::Result<ClientConnection>>(255);

                        // Set the channel
                        app_ctx.connection_receiver = receiver;

                        // Create the connecting thread
                        runtime.spawn_background_task(|_ctx| async move {
                            // Attempt to make a connection to the remote address.
                            let client_connection = ClientConnection::connect_to_address(
                                address,
                                CertificateDer::from_slice(&[]),
                            )
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
                        ui.add(egui::Button::new("Quit").frame(false));
                    });
                });
        }
    }

    // Try receiving the incoming successful connection to the remote address.
    if let Ok(connection) = app_ctx.connection_receiver.try_recv() {
        match connection {
            Ok(valid_connection) => {
                // Set the client connection variable
                app_ctx.client_connection = Some(valid_connection);

                // Set the window to be game
                app_ctx.ui_mode = UiMode::Game;
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
