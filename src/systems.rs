use bevy::{
    asset::Assets,
    color::Color,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        event::EventReader,
        identifier::Identifier,
        query::With,
        system::{Commands, Query, Res, ResMut},
        world::Mut,
    },
    input::{
        keyboard::KeyCode,
        mouse::{MouseButton, MouseButtonInput},
        ButtonInput,
    },
    math::{curve::cores::even_interp, primitives::Circle, vec2},
    render::mesh::{Mesh, Mesh2d},
    sprite::{ColorMaterial, MeshMaterial2d},
    transform::components::Transform,
};

use bevy_egui::{
    egui::{self, Align2, Color32, Layout, RichText},
    EguiContexts,
};
use bevy_rapier2d::
    prelude::{
        ActiveEvents, AdditionalMassProperties, Ccd, CharacterAutostep, Collider, ColliderMassProperties, CollisionGroups, ExternalForce, ExternalImpulse, Group, KinematicCharacterController, LockedAxes, MassProperties, Restitution, RigidBody, Velocity
    };
use punchafriend::{
    ApplicationCtx, AttackObject, CollisionGroupSet, ForeignCharacter, MapElement, SelfCharacter, UiState
};

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
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

    // Create the SelfCharacter.
    commands
        .spawn(RigidBody::Dynamic)
        .insert(Collider::ball(20.0))
        .insert(Transform::from_xyz(0., 100., 0.))
        .insert(LockedAxes::ROTATION_LOCKED)
        .insert(AdditionalMassProperties::Mass(0.1))
        .insert(KinematicCharacterController {
            apply_impulse_to_dynamic_bodies: false,
            ..Default::default()
        })
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(collision_groups.self_character)
        .insert(Ccd::enabled())
        .insert(SelfCharacter::default());

    // Create the ForeignCharacter.
    commands
        .spawn(RigidBody::Dynamic)
        .insert(Collider::ball(20.0))
        .insert(Transform::from_xyz(0., 100., 0.))
        .insert(AdditionalMassProperties::Mass(0.1))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(LockedAxes::ROTATION_LOCKED)
        .insert(collision_groups.foreign_character)
        .insert(Ccd::enabled())
        .insert(ForeignCharacter::default());
}

pub fn frame(
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(
        Entity,
        &mut SelfCharacter,
        &mut KinematicCharacterController,
        &mut Transform,
    )>,
    mut app_ctx: ResMut<ApplicationCtx>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    collision_groups: Res<CollisionGroupSet>,
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

    if let Ok(query) = query.get_single_mut() {
        handle_player_movement(query, commands, keyboard_input, mouse_input, collision_groups);
    }
}

pub fn check_for_collision_with_map(
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    mut self_character_query: Query<&mut SelfCharacter>,
) {
    for collision in collision_events.read() {
        match collision {
            bevy_rapier2d::prelude::CollisionEvent::Started(
                entity,
                entity2,
                collision_event_flags,
            ) => {
                let entity1_p = self_character_query.get(*entity).is_ok();
                let entity1_m = map_element_query.get(*entity).is_ok();
                let entity2_p = self_character_query.get(*entity2).is_ok();
                let entity2_m = map_element_query.get(*entity2).is_ok();

                // Check if entity1 is the player and entity2 is the map element
                if entity1_p && entity2_m {
                    let mut self_character_ref = self_character_query.get_mut(*entity).unwrap();

                    self_character_ref.can_jump = true;
                }

                // Check if entity2 is the player and entity1 is the map element
                if entity1_m && entity2_p {
                    let mut self_character_ref = self_character_query.get_mut(*entity2).unwrap();

                    self_character_ref.can_jump = true;
                }
            }
            bevy_rapier2d::prelude::CollisionEvent::Stopped(
                entity,
                entity1,
                collision_event_flags,
            ) => {}
        }
    }
}

fn handle_player_movement(
    query: (
        Entity,
        Mut<SelfCharacter>,
        Mut<KinematicCharacterController>,
        Mut<Transform>,
    ),
    mut commands: Commands,
    keyboard_input: ButtonInput<KeyCode>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    collision_groups: Res<CollisionGroupSet>,
) {
    let (entity, mut self_character, mut controller, transfrom) = query;

    if keyboard_input.pressed(KeyCode::KeyA) {
        controller.translation = Some(vec2(-1.5, 0.));
    }

    if keyboard_input.pressed(KeyCode::KeyD) {
        controller.translation = Some(vec2(1.5, 0.));
    }

    if keyboard_input.just_pressed(KeyCode::Space) && self_character.can_jump {
        commands.entity(entity).insert(Velocity {
            linvel: vec2(0., 500.),
            angvel: 0.5,
        });

        self_character.can_jump = false;
    }

    if keyboard_input.just_pressed(KeyCode::KeyS) {
        commands.entity(entity).insert(Velocity {
            linvel: vec2(0., -500.),
            angvel: 0.5,
        });
    }

    if mouse_input.just_pressed(MouseButton::Left) {
        // Spawn in a cuboid and then caluclate the collisions from that
        commands
            .spawn(Collider::cuboid(100., 20.))
            .insert(ActiveEvents::COLLISION_EVENTS)
            .insert(ActiveEvents::CONTACT_FORCE_EVENTS)
            .insert(AttackObject)
            .insert(Group::GROUP_3)
            .insert(collision_groups.attack_obj)
            .insert(Transform::from_xyz(
                transfrom.translation.x,
                transfrom.translation.y,
                transfrom.translation.z,
            ));
    }
}

pub fn check_for_collision_with_attack_object(
    mut commands: Commands,
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    foreign_character_query: Query<(Entity, &mut ForeignCharacter)>,
    attack_object_query: Query<(Entity, &AttackObject)>,
) {
    for collision in collision_events.read() {
        match collision {
            bevy_rapier2d::prelude::CollisionEvent::Started(
                entity,
                entity1,
                collision_event_flags,
            ) => {
                let attack_obj_query_result = attack_object_query
                    .iter()
                    .find(|(attck_ent, _)| *attck_ent == *entity || *attck_ent == *entity1);

                let foreign_character_query_result =
                    foreign_character_query
                        .iter()
                        .find(|(foreign_character_entity, _)| {
                            *foreign_character_entity == *entity
                                || *foreign_character_entity == *entity1
                        });

                match (attack_obj_query_result, foreign_character_query_result) {
                    (Some((ent, attack_object)), Some((foreign_entity, mut foreign_character))) => {
                        let mut colliding_entity_commands = commands.entity(foreign_entity);

                        colliding_entity_commands.insert(ExternalImpulse {
                            impulse: vec2(0., 500000.),
                            torque_impulse: 1000000.,
                        });
                    }
                    _ => {}
                };
            }
            bevy_rapier2d::prelude::CollisionEvent::Stopped(
                entity,
                entity1,
                collision_event_flags,
            ) => {}
        };
    }

    //Remove all the attacks objects after checking for collision
    for (ent, _obj) in attack_object_query.iter() {
        commands.entity(ent).despawn();
    }
}

pub fn ui_system(
    mut contexts: EguiContexts,
    mut app_ctx: ResMut<ApplicationCtx>,
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    collision_groups: Res<CollisionGroupSet>,
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
                            setup(commands, meshes, materials, collision_groups);
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
