use std::{net::SocketAddr, time::Duration};

use bevy::{
    asset::Assets,
    ecs::{
        entity::Entity,
        system::{Commands, Query, Res, ResMut},
    },
    render::mesh::Mesh,
    sprite::ColorMaterial,
    time::Timer,
};
use bevy_egui::{
    egui::{self, Align2, Color32, Layout, RichText},
    EguiContexts,
};
use bevy_tokio_tasks::TokioTasksRuntime;
use punchafriend::{
    game::{
        collision::CollisionGroupSet,
        map::{setup_map_from_mapinstance, MapElement, MapName, MapNameDiscriminants},
    },
    networking::{
        server::{send_request_to_client, setup_remote_client_handler, ServerInstance},
        IntermissionData, RemoteServerRequest, ServerGameState,
    },
    server::ApplicationCtx,
    UiLayer,
};
use strum::VariantArray;
use tokio::sync::mpsc::channel;

pub fn ui_system(
    mut contexts: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
    current_map_objects: Query<(Entity, &MapElement)>,
    runtime: ResMut<TokioTasksRuntime>,
) {
    let ctx = contexts.ctx_mut();

    match app_ctx.ui_mode {
        // If there is a game currently playing we should display the HUD.
        punchafriend::UiLayer::Game => {
            egui::SidePanel::left("server_panel").show(ctx, |ui| {
                if let Some(inst) = &app_ctx.server_instance {
                    ui.label(format!("Port: {}", inst.tcp_listener_port));

                    if ui.button("Set intermission state").clicked() {
                        let dash_map = inst.connected_client_game_sockets.clone();

                        let intermission_data = IntermissionData::new(
                            MapNameDiscriminants::VARIANTS.to_vec().iter().map(|map| (*map, 0)).collect::<Vec<(MapNameDiscriminants, usize)>>(),
                            Timer::new(
                                Duration::from_secs(30),
                                bevy::time::TimerMode::Once,
                            ),
                        );

                        if let Some(server_instance) = &app_ctx.server_instance {
                            *server_instance.game_state.write() = ServerGameState::Intermission(intermission_data.clone());
                        }

                        runtime.spawn_background_task(move |_ctx| async move {
                            // These are the sockets which returned an error when reading from them
                            let mut erroring_socket_addresses: Vec<SocketAddr> = vec![];

                            // Get the connected clients list
                            for connected_client in dash_map.iter_mut() {
                                // Get the handle of the TcpStream established when the client was connecting to the server
                                let (_, tcp_stream) = connected_client.value();

                                // Send the disconnection message on the TcpStream specified
                                if let Err(err) = send_request_to_client(
                                    &mut tcp_stream.lock(),
                                    RemoteServerRequest {
                                        request: punchafriend::networking::ServerRequest::ServerGameStateControl(ServerGameState::Intermission(
                                            intermission_data.clone()
                                        ))
                                    },
                                )
                                .await
                                {
                                    dbg!(err);

                                    erroring_socket_addresses.push(*connected_client.key());
                                };
                            }

                            for erroring_socket in &erroring_socket_addresses {
                                dash_map.remove(erroring_socket);
                            }
                        });
                    }
                }
            });
        }
        // Display main menu window.
        punchafriend::UiLayer::MainMenu => {
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
                        ui.add(
                            egui::Button::new(RichText::from("Map Creator").size(25.)).frame(false),
                        );

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

                            app_ctx.ui_mode = UiLayer::Game;
                        };

                        ui.add_space(50.);
                    });
                });
        }
        punchafriend::UiLayer::PauseWindow(_) => {
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
        punchafriend::UiLayer::GameMenu => {}
        punchafriend::UiLayer::Intermission(_) => {
            // unimplemented!();
        }
    }

    if app_ctx.server_instance.is_some() {
        return;
    }

    if let Ok(server_instance) = app_ctx.server_instance_receiver.try_recv() {
        match server_instance {
            Ok(mut server_instance) => {
                // Initalize game
                let game_state = server_instance.game_state.read();

                match game_state.clone() {
                    punchafriend::networking::ServerGameState::Pause => {
                        unimplemented!("The server should never reach this point.");
                    }
                    punchafriend::networking::ServerGameState::Intermission(_) => {
                        unimplemented!("The server should never reach this point.");
                    }
                    punchafriend::networking::ServerGameState::OngoingGame(map_instance) => {
                        setup_map_from_mapinstance(
                            map_instance,
                            commands,
                            collision_groups.clone(),
                            current_map_objects,
                        );
                    }
                }

                drop(game_state);

                // Initalize server threads
                setup_remote_client_handler(
                    &mut server_instance,
                    runtime,
                    app_ctx.cancellation_token.clone(),
                    collision_groups.clone(),
                );

                app_ctx.server_instance = Some(server_instance);
            }
            Err(err) => {}
        }
    }
}
