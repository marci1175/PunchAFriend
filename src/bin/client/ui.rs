use bevy::{
    asset::{AssetId, Assets}, ecs::{
        entity::Entity,
        system::{Commands, Query, Res, ResMut},
    }, input::{keyboard::KeyCode, ButtonInput}, math::UVec2, render::mesh::Mesh, sprite::TextureAtlasLayout, time::Time, transform::components::Transform
};
use bevy_egui::{
    egui::{self, vec2, Align2, Color32, Grid, Layout, Pos2, RichText, Sense, Slider},
    EguiContexts,
};
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_tokio_tasks::TokioTasksRuntime;

use chrono::Local;
use egui_extras::{Column, TableBuilder};
use punchafriend::{
    client::ApplicationCtx,
    game::{collision::CollisionGroupSet, pawns::Pawn},
    networking::{client::ClientConnection, RemoteClientRequest},
    PauseWindowState, UiLayer,
};

use crate::systems::reset_connection_and_ui;

pub fn ui_system(
    mut context: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    runtime: ResMut<TokioTasksRuntime>,
    players: Query<(Entity, &mut Pawn, &mut Transform)>,
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<TextureAtlasLayout>>,
    collision_groups: Res<CollisionGroupSet>,
    mut framepace: ResMut<FramepaceSettings>,
) {
    // Get context
    let ctx = context.ctx_mut();

    // Install all image loaders
    egui_extras::install_image_loaders(ctx);

    // Show toasts
    app_ctx.egui_toasts.show(ctx);

    let local_utc_time = Local::now().to_utc();

    // Match the UiLayer enum's state
    match app_ctx.ui_layer.clone() {
        UiLayer::Game(ongoing_game_data) => {
            // How much time is left from the round
            let time_delta = ongoing_game_data
                .round_end_date
                .time()
                .signed_duration_since(local_utc_time.time());

            egui::Area::new("hud".into())
                .anchor(Align2::CENTER_TOP, vec2(0., 20.))
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Round time: {:.2}s",
                        time_delta.num_milliseconds() as f32 / 1000.
                    ));
                });

            // Set the new value of the UiLayer's enum
            app_ctx.ui_layer = UiLayer::Game(ongoing_game_data.clone());

            if keyboard_input.pressed(KeyCode::Tab) {
                let leaderboard_area = egui::Area::new("scoreboard".into())
                    .anchor(Align2::CENTER_CENTER, vec2(0., 0.))
                    .show(ctx, |ui| {
                        if let Some(connection) = &app_ctx.client_connection {
                            ui.painter().rect_filled(
                                app_ctx.ui_state.leaderboard_rect,
                                3.,
                                Color32::from_black_alpha(210),
                            );

                            ui.group(|ui| {
                                let table = TableBuilder::new(ui)
                                    .striped(true)
                                    .columns(Column::auto(), 5)
                                    .cell_layout(Layout::left_to_right(egui::Align::Center));

                                table
                                    .header(20., |mut header| {
                                        header.col(|ui| {
                                            ui.label("Username");
                                        });
                                        header.col(|ui| {
                                            ui.label("Kills");
                                        });
                                        header.col(|ui| {
                                            ui.label("Deaths");
                                        });
                                        header.col(|ui| {
                                            ui.label("Score");
                                        });
                                        header.col(|ui| {
                                            ui.label("K/D");
                                        });
                                    })
                                    .body(|body| {
                                        let client_stats =
                                            connection.connected_clients_stats.read().clone();
                                        let mut client_stats_iter = client_stats.iter();

                                        body.rows(
                                            20.,
                                            connection.connected_clients_stats.read().len(),
                                            |mut column| {
                                                if let Some(client) = client_stats_iter.next() {
                                                    column.col(|ui| {
                                                        ui.label(client.username.clone());
                                                    });
                                                    column.col(|ui| {
                                                        ui.label(format!("{}", client.kills));
                                                    });
                                                    column.col(|ui| {
                                                        ui.label(format!("{}", client.deaths));
                                                    });
                                                    column.col(|ui| {
                                                        ui.label(format!("{}", client.score));
                                                    });
                                                    column.col(|ui| {
                                                        ui.label(format!(
                                                            "{:.2}",
                                                            client.kills as f32
                                                                / client.deaths as f32
                                                        ));
                                                    });
                                                }
                                            },
                                        );
                                    });
                            });
                        }
                    });

                app_ctx.ui_state.leaderboard_rect = leaderboard_area.response.rect;
            }
        }
        UiLayer::Intermission(intermission_data) => {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::from("Vote for the next map!").size(20.).strong());

                    ui.label(format!(
                        "Time left: {}s",
                        intermission_data.intermission_end_date.time().signed_duration_since(local_utc_time.time()).num_seconds()
                    ));
                });

                Grid::new("map_grid").show(ui, |ui| {
                    // Iter over all the available maps
                    for (map_idx, (map, vote_count)) in intermission_data.selectable_maps.iter().enumerate() {
                        // Display the group
                            ui.group(|ui| {
                                // Allocate ui
                                ui.allocate_ui(vec2(100., 100.), |ui| {
                                    ui.vertical_centered(|ui| {
                                        // Display the map's name
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::from(map.to_string()).strong());
                                            ui.label(RichText::from(vote_count.to_string()).strong());
                                        });
    
                                        // Display an image of the map
                                        ui.image(egui::include_image!(
                                            "../../../assets/map_imgs/test.png"
                                        ));
    
                                        // Show the vote button as available if the user hasnt voted yet.
                                        ui.add_enabled_ui(!app_ctx.has_voted, |ui| {
                                            // Show the button to vote
                                            if ui.button("Vote").clicked() {
                                                if let Some(client_connection) = &app_ctx.client_connection {
                                                    client_connection.remote_server_sender.try_send(RemoteClientRequest {
                                                        id: client_connection.server_metadata.client_uuid,
                                                        request: punchafriend::networking::ClientRequest::Vote(*map),
                                                    }).unwrap();
                                                }
    
                                                // Prevent the user for voting multiple times
                                                app_ctx.has_voted = true;
                                            };
                                        });
                                    });
                                });
                            });

                        // End the row every 5 maps
                        if map_idx % 5 == 0 {
                            ui.end_row();
                        }
                    }
                });

                ui.separator();
            });

            // Set the innter value of the ui_layer
            app_ctx.ui_layer = UiLayer::Intermission(intermission_data);
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

                    ui.label("Set Username");

                    // Username buffer setter
                    ui.text_edit_singleline(&mut app_ctx.ui_state.username_buffer);
                    
                    ui.add_enabled_ui(!app_ctx.ui_state.username_buffer.is_empty(), |ui| {

                    });

                    if ui.button("Connect").clicked() && app_ctx.client_connection.is_none() {
                        // Clone the address so it can be moved.
                        let address = app_ctx.ui_state.connect_to_address.clone();

                        // Move the sender
                        let sender = app_ctx.connection_sender.clone();

                        // Set the channel
                        let cancellation_token = app_ctx.cancellation_token.clone();

                        let username = app_ctx.ui_state.username_buffer.clone();

                        // Create the connecting thread
                        runtime.spawn_background_task(|_ctx| async move {
                            // Attempt to make a connection to the remote address.
                            let client_connection =
                                ClientConnection::connect_to_address(address, username.clone(), cancellation_token)
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
                                    app_ctx.ui_layer = *state_before.clone();
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

                        ui.horizontal(|ui| {
                            ui.label("Textures");

                            if ui.button("Reload all Textures").clicked() {
                                for material in materials
                                    .iter()
                                    .map(|(mat, _la)| mat)
                                    .collect::<Vec<AssetId<TextureAtlasLayout>>>()
                                    .into_iter()
                                {
                                    
                                    materials.remove(material);
                                }

                                app_ctx.texture_atlas_layouts = materials.add(TextureAtlasLayout::from_grid(
                                    UVec2::new(50, 64),
                                    7,
                                    1,
                                    Some(UVec2::new(20, 0)),
                                    None,
                                ));
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
