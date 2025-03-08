use bevy::{
    asset::Assets,
    ecs::system::{Commands, Res, ResMut},
    render::mesh::Mesh,
    sprite::ColorMaterial,
    time::Time,
};
use bevy_egui::{
    egui::{self, Align2, Color32, Layout, RichText},
    EguiContexts,
};
use bevy_tokio_tasks::TokioTasksRuntime;
use punchafriend::{
    game::collision::CollisionGroupSet,
    networking::server::{setup_remote_client_handler, ServerInstance},
    server::ApplicationCtx,
};
use tokio::sync::mpsc::channel;

use crate::systems::setup_game;

pub fn ui_system(
    mut contexts: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
    // mut local_player: Query<&mut LocalPlayer>,
    runtime: ResMut<TokioTasksRuntime>,
    time: Res<Time>,
) {
    let ctx = contexts.ctx_mut();

    match app_ctx.ui_mode {
        // If there is a game currently playing we should display the HUD.
        punchafriend::UiMode::Game => {}
        // Display main menu window.
        punchafriend::UiMode::MainMenu => {
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
                            // Create a new pair of channels
                            let (sender, receiver) = channel::<anyhow::Result<ServerInstance>>(255);

                            // Set the receiver so that it will receive the new instnace from the async task
                            app_ctx.server_instance_receiver = receiver;

                            // Spawn a new async task
                            runtime.spawn_background_task(|_ctx| async move {
                                // Create a new ServerInstance
                                let connection_result = ServerInstance::create_server().await;

                                // Send the new instance through the channel
                                sender.send(connection_result).await.unwrap();
                            });
                        };

                        if let Some(inst) = &app_ctx.server_instance {
                            ui.label(format!("{}", inst.tcp_listener_port));
                        }

                        ui.add_space(50.);
                    });
                });
        }
        punchafriend::UiMode::PauseWindow => {
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
        punchafriend::UiMode::GameMenu => {}
    }

    if app_ctx.server_instance.is_some() {
        return;
    }

    if let Ok(server_instance) = app_ctx.server_instance_receiver.try_recv() {
        match server_instance {
            Ok(server_instance) => {
                app_ctx.server_instance = Some(server_instance.clone());
                // Initalize game
                setup_game(commands, meshes, materials, &collision_groups);

                // Initalize server threads
                setup_remote_client_handler(
                    server_instance,
                    runtime,
                    app_ctx.cancellation_token.clone(),
                    collision_groups.clone(),
                );
            }
            Err(err) => {}
        }
    }
}
