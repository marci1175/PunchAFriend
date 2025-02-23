use bevy::{
    asset::Assets,
    color::Color,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity, event::EventReader, identifier::Identifier, query::With, system::{Commands, Query, Res, ResMut}, world::Mut
    },
    input::{keyboard::KeyCode, ButtonInput},
    math::{curve::cores::even_interp, primitives::Circle, vec2},
    render::mesh::{Mesh, Mesh2d},
    sprite::{ColorMaterial, MeshMaterial2d},
    transform::components::Transform,
};

use bevy_egui::{
    egui::{self, Align2, Color32, Layout, RichText},
    EguiContexts,
};
use bevy_rapier2d::{prelude::{AdditionalMassProperties, Collider, ColliderMassProperties, ExternalForce, ExternalImpulse, KinematicCharacterController, MassProperties, Restitution, RigidBody, Velocity}, rapier::prelude::{CollisionEvent, ContactForceEvent}};
use punchafriend::{ApplicationCtx, MapElement, SelfCharacter, UiState};

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    // Setup graphics
    commands.spawn(Camera2d);

    commands
        .spawn(Collider::cuboid(500.0, 10.0))
        .insert(Transform::from_xyz(0.0, -200.0, 0.0))
        .insert(MapElement);

    /* Create the bouncing ball. */
    commands
        .spawn(RigidBody::Dynamic)
        .insert(Collider::ball(20.0))
        .insert(Transform::from_xyz(0., 100., 0.))
        .insert(AdditionalMassProperties::Mass(0.1))
        .insert(KinematicCharacterController::default())
        .insert(SelfCharacter);
}

pub fn frame(
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(Entity, &mut KinematicCharacterController), With<SelfCharacter>>,
    mut app_ctx: ResMut<ApplicationCtx>,
) {
    let keyboard_input = keyboard_input.clone();

    if keyboard_input.just_pressed(KeyCode::Escape) {
        if app_ctx.ui_state == UiState::PauseWindow {
            app_ctx.ui_state = UiState::Game;
        } else {
            app_ctx.ui_state = UiState::PauseWindow;
        }
    }

    // If we the ui isnt in `Game` state, do not let the user interact with the game.
    if app_ctx.ui_state != UiState::Game {
        return;
    }

    if let Ok(entity) = query.get_single_mut() {
        handle_player_movement(entity, commands, keyboard_input);
    }
}

pub fn check_for_collision(
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    self_character_query: Query<Entity, With<SelfCharacter>>,
) {
    for collision in collision_events.read() {
        match collision {
            bevy_rapier2d::prelude::CollisionEvent::Started(entity, entity2, collision_event_flags) => {
                map_element_query.get(*entity);
                self_character_query.get(*entity2);
            },
            bevy_rapier2d::prelude::CollisionEvent::Stopped(entity, entity1, collision_event_flags) => {

            },
        }
    }
}

fn handle_player_movement(
    query: (Entity, Mut<KinematicCharacterController>),
    mut commands: Commands,
    keyboard_input: ButtonInput<KeyCode>,
) {
    let (entity, mut controller) = query;

    if keyboard_input.pressed(KeyCode::KeyA) {
        controller.translation = Some(vec2(-1.5, 0.));
    }

    if keyboard_input.pressed(KeyCode::KeyD) {
        controller.translation = Some(vec2(1.5, 0.));
    }

    if keyboard_input.just_pressed(KeyCode::Space) {
        commands.entity(entity).insert(Velocity {
            linvel: vec2(0., 500.),
            angvel: 0.5,
        });
    }

    if keyboard_input.just_pressed(KeyCode::KeyS) {
        commands.entity(entity).insert(Velocity {
            linvel: vec2(0., -500.),
            angvel: 0.5,
        });
    }
}

pub fn ui_system(
    mut contexts: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
) {
    let ctx = contexts.ctx_mut();

    match app_ctx.ui_state {
        // If there is a game currently playing we should display the HUD.
        punchafriend::UiState::Game => {}
        // Display main menu window.
        punchafriend::UiState::MainMenu => {
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
                            app_ctx.ui_state = UiState::Game;

                            // Initalize game
                            setup(commands, meshes, materials);
                        };

                        ui.add_space(50.);
                    });
                });
        }
        punchafriend::UiState::PauseWindow => {
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
}
