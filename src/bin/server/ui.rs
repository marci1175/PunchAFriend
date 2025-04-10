use std::{net::SocketAddr, sync::Arc, time::Duration};

use bevy::{
    asset::Assets,
    ecs::{
        entity::Entity,
        query::Without,
        system::{Commands, Query, Res, ResMut},
    },
    render::mesh::Mesh,
    sprite::ColorMaterial,
    time::Timer,
    transform::components::Transform,
};
use bevy_egui::{
    egui::{self, Align2, Color32, Layout, RichText},
    EguiContexts,
};
use bevy_tokio_tasks::TokioTasksRuntime;
use chrono::{Local, TimeDelta};
use dashmap::DashMap;
use parking_lot::Mutex;
use punchafriend::{
    game::{
        collision::CollisionGroupSet,
        map::{load_map_from_mapinstance, MapElement, MapNameDiscriminants, MapObjectUpdate},
        pawns::Pawn,
    },
    networking::{
        server::{send_request_to_client, setup_remote_client_handler, ServerInstance},
        IntermissionData, RemoteServerRequest, ServerGameState, ServerTickUpdate,
    },
    server::ApplicationCtx,
    UiLayer,
};
use strum::VariantArray;
use tokio::{
    net::{tcp::OwnedWriteHalf, UdpSocket},
    sync::mpsc::channel,
};
use uuid::Uuid;

use crate::systems::MINUTE_SECS;

pub fn ui_system(
    mut contexts: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    mut commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
    current_map_objects: Query<(Entity, &MapElement, &mut Transform), Without<Pawn>>,
    runtime: ResMut<TokioTasksRuntime>,
) {
    let ctx = contexts.ctx_mut();

    match app_ctx.ui_mode.clone() {
        // If there is a game currently playing we should display the HUD.
        punchafriend::UiLayer::Game(_ongoing_game_data) => {
            egui::SidePanel::left("server_panel").show(ctx, |ui| {
                if let Some(inst) = &app_ctx.server_instance {
                    if let Some(round_timer) = &app_ctx.game_round_timer {
                        ui.label(format!(
                            "Time left: {:.2}s",
                            round_timer.duration().as_secs_f32() - round_timer.elapsed_secs()
                        ));
                    }

                    ui.label(format!("Port: {}", inst.tcp_listener_port));

                    if ui.button("Set intermission state").clicked() {
                        let dash_map = inst.connected_client_tcp_handles.clone();

                        let intermission_data = create_intermission_data_all();

                        if let Some(server_instance) = &app_ctx.server_instance {
                            *server_instance.game_state.write() =
                                ServerGameState::Intermission(intermission_data.clone());
                        }

                        app_ctx.intermission_timer = Some(Timer::new(
                            Duration::from_secs(30),
                            bevy::time::TimerMode::Once,
                        ));

                        notify_valid_clients_intermission(&runtime, dash_map, intermission_data);
                    }

                    ui.separator();

                    ui.label("Game Timers:");

                    if let Some(intermission_timer) = &app_ctx.intermission_timer {
                        ui.label(format!(
                            "Intermission elapsed: {:.2}s",
                            intermission_timer.elapsed_secs()
                        ));
                    }

                    if let Some(game_round_timer) = &app_ctx.game_round_timer {
                        ui.label(format!(
                            "Game round elapsed: {:.2}s",
                            game_round_timer.elapsed_secs()
                        ));
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
                    punchafriend::networking::ServerGameState::OngoingGame(game_data) => {
                        load_map_from_mapinstance(
                            game_data.current_map.clone(),
                            &mut commands,
                            collision_groups.clone(),
                            current_map_objects,
                        );

                        app_ctx.ui_mode = UiLayer::Game(game_data.clone());
                    }
                }

                // Reset the round timer's state
                app_ctx.game_round_timer = Some(Timer::new(
                    Duration::from_secs(MINUTE_SECS * 8),
                    bevy::time::TimerMode::Once,
                ));

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

pub fn create_intermission_data_all() -> IntermissionData {
    let intermission_data = IntermissionData::new(
        MapNameDiscriminants::VARIANTS
            .to_vec()
            .iter()
            .map(|map| (*map, 0))
            .collect::<Vec<(MapNameDiscriminants, usize)>>(),
        Local::now()
            .to_utc()
            .checked_add_signed(TimeDelta::try_seconds(30).unwrap())
            .unwrap(),
    );
    intermission_data
}

pub fn notify_valid_clients_map_change(
    udp_socket: Arc<UdpSocket>,
    runtime: &Res<'_, TokioTasksRuntime>,
    dash_map: Arc<DashMap<SocketAddr, (Uuid, Arc<Mutex<OwnedWriteHalf>>)>>,
    map_object_update: MapObjectUpdate,
) {
    runtime.spawn_background_task(move |_ctx| async move {
        // Serialize the packet into bytes so it can be sent later
        let message_bytes = rmp_serde::to_vec(&ServerTickUpdate::new(
            punchafriend::networking::TickUpdateType::MapObject(map_object_update),
        ))
        .unwrap();

        // Get the lenght of the message and turn it into bytes
        let mut message_length_bytes = (message_bytes.len() as u32).to_be_bytes().to_vec();

        // Append the message bytes to the header
        message_length_bytes.extend(message_bytes.clone());

        // Get the connected clients list
        for socket_addr in dash_map.iter() {
            // Get the handle of the TcpStream established when the client was connecting to the server
            let socket_addr = socket_addr.key();

            udp_socket
                .send_to(&message_length_bytes, *socket_addr)
                .await
                .unwrap();
        }
    });
}

pub fn notify_valid_clients_intermission(
    runtime: &ResMut<'_, TokioTasksRuntime>,
    dash_map: std::sync::Arc<
        dashmap::DashMap<
            SocketAddr,
            (
                uuid::Uuid,
                std::sync::Arc<
                    parking_lot::lock_api::Mutex<
                        parking_lot::RawMutex,
                        tokio::net::tcp::OwnedWriteHalf,
                    >,
                >,
            ),
        >,
    >,
    intermission_data: IntermissionData,
) {
    runtime.spawn_background_task(move |_ctx| async move {
        // These are the sockets which returned an error when reading from them
        let mut erroring_socket_addresses: Vec<SocketAddr> = vec![];

        // Get the connected clients list
        for connected_client in dash_map.iter_mut() {
            // Get the handle of the TcpStream established when the client was connecting to the server
            let (_, write_half) = connected_client.value();

            // Send the disconnection message on the TcpStream specified
            if let Err(err) = send_request_to_client(
                &mut write_half.lock(),
                RemoteServerRequest {
                    request: punchafriend::networking::ServerRequest::ServerGameStateControl(
                        ServerGameState::Intermission(intermission_data.clone()),
                    ),
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
