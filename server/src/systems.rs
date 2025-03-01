use std::time::Duration;

use bevy::{
    asset::Assets,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        event::EventReader,
        query::With,
        system::{Commands, Query, Res, ResMut},
    },
    input::{keyboard::KeyCode, ButtonInput},
    math::vec2,
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
    ActiveEvents, AdditionalMassProperties, Ccd, Collider, KinematicCharacterController,
    LockedAxes, RigidBody, Velocity,
};
use server::{
    game::{
        combat::{AttackObject, AttackType, Combo},
        pawns::{
            local_player_handle, LocalPlayer, Player
        },
    },
    ApplicationCtx, CollisionGroupSet, Direction, MapElement, UiState,
};

pub fn setup(
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
        .insert(collision_groups.local_player)
        .insert(Ccd::enabled())
        // We add the LocalPlayer bundle to the entity, so we can differentiate the entities to the one we control.
        .insert(LocalPlayer::default());

    // Create the ForeignCharacter, but only in debug mode.
    #[cfg(debug_assertions)]
    commands
        .spawn(RigidBody::Dynamic)
        .insert(Collider::ball(20.0))
        .insert(Transform::from_xyz(0., 100., 0.))
        .insert(AdditionalMassProperties::Mass(0.1))
        .insert(ActiveEvents::COLLISION_EVENTS)
        .insert(LockedAxes::ROTATION_LOCKED)
        .insert(collision_groups.player)
        .insert(Ccd::enabled())
        .insert(Velocity::default())
        .insert(Player::default());
}

pub fn frame(
    commands: Commands,
    meshes: ResMut<Assets<Mesh>>,
    materials: ResMut<Assets<ColorMaterial>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<(
        Entity,
        &mut LocalPlayer,
        &mut KinematicCharacterController,
        &Transform,
    )>,
    mut app_ctx: ResMut<ApplicationCtx>,
    collision_groups: Res<CollisionGroupSet>,
    time: Res<Time>,
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
        local_player_handle(
            query,
            commands,
            keyboard_input,
            collision_groups,
            app_ctx,
            time,
        );
    }
}

pub fn reset_jump_remaining_for_local_player(
    collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    character_entity_query: Query<Entity, With<LocalPlayer>>,
    mut local_player_query: Query<&mut LocalPlayer>,
) {
    if let Some(colliding_entity) = check_for_collision_with_map_and_selfcharacter(
        collision_events,
        map_element_query,
        character_entity_query,
    ) {
        if let Ok(mut local_player) = local_player_query.get_mut(colliding_entity) {
            local_player.jumps_remaining = 2;
        }
    }
}

pub fn check_for_collision_with_map_and_selfcharacter(
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    character_entity_query: Query<Entity, With<LocalPlayer>>,
) -> Option<Entity> {
    if let Some(collision) = collision_events.read().next() {
        match collision {
            bevy_rapier2d::prelude::CollisionEvent::Started(
                entity,
                entity2,
                _collision_event_flags,
            ) => {
                let entity1_p = character_entity_query.get(*entity);
                let entity1_m = map_element_query.get(*entity);
                let entity2_p = character_entity_query.get(*entity2);
                let entity2_m = map_element_query.get(*entity2);

                // Check if entity1 is the player and entity2 is the map element or if entity2 is the player and entity1 is the map element
                return if entity1_p.is_ok() && entity2_m.is_ok() {
                    Some(entity1_p.unwrap())
                } else if entity2_p.is_ok() && entity1_m.is_ok() {
                    Some(entity2_p.unwrap())
                } else {
                    None
                };
            }
            bevy_rapier2d::prelude::CollisionEvent::Stopped(
                entity,
                entity2,
                _collision_event_flags,
            ) => {
                let entity1_p = character_entity_query.get(*entity);
                let entity1_m = map_element_query.get(*entity);
                let entity2_p = character_entity_query.get(*entity2);
                let entity2_m = map_element_query.get(*entity2);

                // Check if entity1 is the player and entity2 is the map element or if entity2 is the player and entity1 is the map element
                return if entity1_p.is_ok() && entity2_m.is_ok() {
                    Some(entity1_p.unwrap())
                } else if entity2_p.is_ok() && entity1_m.is_ok() {
                    Some(entity2_p.unwrap())
                } else {
                    None
                };
            }
        }
    }

    None
}

pub fn check_for_collision_with_attack_object(
    mut commands: Commands,
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    foreign_character_query: Query<(Entity, &mut Player, &Transform, &Velocity)>,
    attack_object_query: Query<(Entity, &AttackObject)>,
    mut local_player: Query<&mut LocalPlayer>,
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
                        .find(|(foreign_character_entity, _, _, _)| {
                            *foreign_character_entity == *entity
                                || *foreign_character_entity == *entity1
                        });

                if let (
                    Some((_attack_ent, attack_object)),
                    Some((foreign_entity, _, foreign_char_transform, foreign_char_velocity)),
                ) = (attack_obj_query_result, foreign_character_query_result)
                {
                    let mut colliding_entity_commands = commands.entity(foreign_entity);

                    let attacker_origin_pos = attack_object.attack_origin.translation;
                    let foreign_char_pos = foreign_char_transform.translation;

                    // Decide the direction the enemy should go
                    // If the attacker is closer to the platforms center it should push the enemy the opposite way.
                    let push_left = if attacker_origin_pos.x > foreign_char_pos.x {
                        -1.0
                    } else {
                        1.0
                    };

                    // Increment the local player's combo counter and reset its timer
                    if let Ok(mut local_player) = local_player.get_mut(attack_object.attack_by) {
                        if let Some(combo_counter) = &mut local_player.combo_stats {
                            combo_counter.combo_counter += 1;
                            combo_counter.combo_timer.reset();
                        } else {
                            local_player.combo_stats = Some(Combo::new(Duration::from_secs(2)));
                        }
                    }

                    colliding_entity_commands.insert(Velocity {
                        linvel: vec2(
                            foreign_char_velocity.linvel.x + 180. * push_left,
                            foreign_char_velocity.linvel.y
                                + if attack_object.attack_type
                                    == AttackType::Directional(Direction::Up)
                                {
                                    500.
                                } else if attack_object.attack_type
                                    == AttackType::Directional(Direction::Down)
                                {
                                    -500.
                                } else {
                                    0.
                                },
                        ),
                        // Angles are disabled
                        angvel: 0.,
                    });
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
    for (ent, _) in attack_object_query.iter() {
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
    mut local_player: Query<&mut LocalPlayer>,
    time: Res<Time>,
) {
    let ctx = contexts.ctx_mut();

    match app_ctx.ui_state {
        // If there is a game currently playing we should display the HUD.
        server::UiState::Game => {
            let local_player = local_player.get_single_mut();

            if let Ok(mut local_player) = local_player {
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
        // Display main menu window.
        server::UiState::MainMenu => {
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
        server::UiState::PauseWindow => {
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
