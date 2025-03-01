use std::{net::{IpAddr, SocketAddr}, sync::Arc, time::Duration};

use bevy::ecs::system::{ResMut, Resource};
use bevy_egui::{
    EguiContexts,
    egui::{self, Align2, Color32, Layout, RichText},
};
use bevy_tokio_tasks::TokioTasksRuntime;
use egui_toast::{Toast, ToastOptions, Toasts};
use parking_lot::Mutex;
use rand::{SeedableRng, rngs::SmallRng};
use tokio::{net::{TcpStream, UdpSocket}, sync::mpsc::{channel, Receiver}};

#[derive(Resource)]
pub struct ClientConnection {
    pub tcp_connection_handle: Arc<Mutex<TcpStream>>,
    pub game_connection_handle: Arc<Mutex<UdpSocket>>,
}

impl ClientConnection {
    pub async fn connect_to_address(address: String) -> anyhow::Result<Self> {
        // Parse socket address.
        let mut address: SocketAddr = address.parse()?;
        
        // Create a new TcpStream instance.
        let tcp_stream = TcpStream::connect(address).await?;

        // Bind to a local address.
        let udp_socket = UdpSocket::bind("[::]:0").await?;

        // Modify the SocketAddr instance.
        address.set_port(address.port() + 1);

        // Set the default desitnation of the socket
        udp_socket.connect(address).await?;

        let game_connection_handle = Arc::new(Mutex::new(udp_socket));
        let tcp_connection_handle = Arc::new(Mutex::new(tcp_stream));

        Ok(ClientConnection { tcp_connection_handle, game_connection_handle })
    }
}

#[derive(Resource)]
pub struct ApplicationCtx {
    /// The Ui's mode in the Application.
    pub ui_mode: UiMode,

    /// The Ui's state in the Application,
    pub ui_state: UiState,

    /// Startup initalized [`SmallRng`] random generator.
    /// Please note, that the [`SmallRng`] is insecure and should not be used in crypto contexts.
    pub rand: rand::rngs::SmallRng,

    /// The Client's currently ongoing connection to a remote address.
    pub client_connection: Option<ClientConnection>,

    /// Receives the connecting threads connection result.
    pub connection_receiver: Receiver<anyhow::Result<ClientConnection>>,

    /// Used to display notifications with egui
    pub egui_toasts: Toasts,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    Game,
    #[default]
    MainMenu,
    GameMenu,
    PauseWindow,
}

#[derive(Debug, Clone)]
pub struct UiState {
    connect_to_address: String,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            connect_to_address: String::new(),
        }
    }
}

impl Default for ApplicationCtx {
    fn default() -> Self {
        Self {
            ui_mode: UiMode::MainMenu,
            ui_state: UiState::default(),
            client_connection: None,
            rand: SmallRng::from_rng(&mut rand::rng()),
            connection_receiver: channel(255).1,
            egui_toasts: Toasts::new(),
        }
    }
}

pub fn ui_system(mut context: EguiContexts, mut app_ctx: ResMut<ApplicationCtx>, runtime: ResMut<TokioTasksRuntime>) {
    // Get context
    let ctx = context.ctx_mut();
    
    // Show toasts
    app_ctx.egui_toasts.show(ctx);

    match app_ctx.ui_mode {
        UiMode::Game => {}
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
                            let client_connection = ClientConnection::connect_to_address(address).await;

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

    if let Ok(connection) = app_ctx.connection_receiver.try_recv() {
        match connection {
            Ok(valid_connection) => {
                app_ctx.client_connection = Some(valid_connection);
            },
            Err(error) => {
                app_ctx.egui_toasts.add(Toast::new().kind(egui_toast::ToastKind::Error).text(format!("Connection Failed: {}", error.to_string())).options(ToastOptions::default().duration(Some(Duration::from_secs(3))).show_progress(true)));
            },
        }
    }
}
