
use bevy::{
    asset::Assets, ecs::{
        entity::Entity, system::{Commands, Query, Res, ResMut}
    }, input::{keyboard::KeyCode, ButtonInput}, render::mesh::Mesh, sprite::TextureAtlasLayout, time::Time, transform::components::Transform
};
use bevy_egui::{
    egui::{self, Align2, Color32, Layout, Pos2, RichText, Sense, Slider},
    EguiContexts,
};
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_tokio_tasks::TokioTasksRuntime;

use punchafriend::{
    client::ApplicationCtx,
    game::{collision::CollisionGroupSet, pawns::Player},
    networking::client::ClientConnection, PauseWindowState, UiLayer,
};

use crate::systems::reset_connection_and_ui;

pub fn ui_system(
    mut context: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    runtime: ResMut<TokioTasksRuntime>,
    players: Query<(Entity, &mut Player, &mut Transform)>,
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<TextureAtlasLayout>>,
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
