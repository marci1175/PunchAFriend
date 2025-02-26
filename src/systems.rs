use bevy::{
    asset::Assets,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        entity::Entity,
        event::EventReader,
        query::With,
        system::{Commands, Query, Res, ResMut},
        world::Mut,
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
use bevy_rapier2d::{prelude::{
    ActiveEvents, AdditionalMassProperties, Ccd, Collider, CollisionEvent, ExternalImpulse, Group, KinematicCharacterController, LockedAxes, Restitution, RigidBody, Velocity
}, rapier::prelude::CollisionEventFlags};
use punchafriend::{
    ApplicationCtx, AttackObject, AttackType, CollisionGroupSet, Direction, ForeignCharacter,
    MapElement, SelfCharacter, UiState,
};
use rand::Rng;
use tokio::time::error::Elapsed;

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
        .insert(Velocity::default())
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
        handle_player_movement(
            query,
            commands,
            keyboard_input,
            collision_groups,
            app_ctx,
            time,
        );
    }
}

pub fn reset_jump_remaining_for_self_chrac(
    collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    character_entity_query: Query<Entity, With<SelfCharacter>>,
    mut self_character_query: Query<&mut SelfCharacter>,
) {
    if let Some(colliding_entity) = check_for_collision_with_map_and_selfcharacter(collision_events, map_element_query, character_entity_query) {
        if let Ok((mut self_character)) = self_character_query.get_mut(colliding_entity) {
            self_character.jumps_remaining = 2;
        }
    }
}

pub fn check_for_collision_with_map_and_selfcharacter(
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    map_element_query: Query<Entity, With<MapElement>>,
    character_entity_query: Query<Entity, With<SelfCharacter>>,
) -> Option<Entity> {
    for collision in collision_events.read() {
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
                    Some(entity1_p.unwrap().clone())
                }
                else if entity2_p.is_ok() && entity1_m.is_ok() {
                    Some(entity2_p.unwrap().clone())
                }
                else {
                    None
                }          
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
                    Some(entity1_p.unwrap().clone())
                }
                else if entity2_p.is_ok() && entity1_m.is_ok() {
                    Some(entity2_p.unwrap().clone())
                }
                else {
                    None
                }
            }
        }
    }

    None
}

fn handle_player_movement(
    query: (
        Entity,
        Mut<SelfCharacter>,
        Mut<KinematicCharacterController>,
        &Transform,
    ),
    mut commands: Commands,
    keyboard_input: ButtonInput<KeyCode>,
    collision_groups: Res<CollisionGroupSet>,
    mut app_ctx: ResMut<ApplicationCtx>,
    time: Res<Time>,
) {
    //
    let (entity, mut self_character, mut controller, transform) = query;

    if keyboard_input.pressed(KeyCode::KeyA) {
        // Move the local player to the left
        controller.translation = Some(vec2(-450. * time.delta_secs(), 0.));
    }

    if keyboard_input.pressed(KeyCode::KeyD) {
        // Move the local player to the right
        controller.translation = Some(vec2(450. * time.delta_secs(), 0.));
    }

    if keyboard_input.just_pressed(KeyCode::KeyD) {
        // Update latest direction
        self_character.direction = Direction::Right;
    }

    if keyboard_input.just_pressed(KeyCode::KeyA) {
        // Update latest direction
        self_character.direction = Direction::Left;
    }

    if keyboard_input.just_pressed(KeyCode::KeyW) {
        // Update latest direction
        self_character.direction = Direction::Up;
    }

    // If the user presses W we the entity should jump, and subtract 1 from the jumps_remaining counter.
    // If there are no more jumps remaining the user needs to wait until they touch a MapObject again. This indicates they've landed.
    // If the user is holding W the entitiy should automaticly jump once on the ground.
    if keyboard_input.just_pressed(KeyCode::KeyW) && self_character.jumps_remaining != 0
        || keyboard_input.pressed(KeyCode::KeyW) && self_character.jumps_remaining == 2
    {
        commands.entity(entity).insert(Velocity {
            linvel: vec2(0., 500.),
            angvel: 0.5,
        });

        self_character.jumps_remaining -= 1;
    }

    if keyboard_input.just_pressed(KeyCode::KeyS) {
        commands.entity(entity).insert(Velocity {
            linvel: vec2(0., -500.),
            angvel: 0.5,
        });

        // Update latest direction
        self_character.direction = Direction::Down;
    }

    // if the player is attacking
    if keyboard_input.just_pressed(KeyCode::Space) {
        let (attack_collider_width, attack_collider_height) = (50., 50.);
        let attack_collider = Collider::cuboid(attack_collider_width, attack_collider_height);

        let attack_transform = match self_character.direction {
            Direction::Left => Transform::from_xyz(
                transform.translation.x - attack_collider_width,
                transform.translation.y,
                0.,
            ),
            Direction::Right => Transform::from_xyz(
                transform.translation.x + attack_collider_width,
                transform.translation.y,
                0.,
            ),
            Direction::Up => Transform::from_xyz(
                transform.translation.x,
                transform.translation.y + attack_collider_height,
                0.,
            ),
            Direction::Down => Transform::from_xyz(
                transform.translation.x,
                transform.translation.y - attack_collider_height,
                0.,
            ),
        };

        // Spawn in a cuboid and then caluclate the collisions from that
        commands
            .spawn(attack_collider)
            .insert(ActiveEvents::COLLISION_EVENTS)
            .insert(ActiveEvents::CONTACT_FORCE_EVENTS)
            .insert(AttackObject::new(
                punchafriend::AttackType::Directional(self_character.direction),
                app_ctx.rand.random_range(14.0..21.0),
                *transform,
            ))
            .insert(collision_groups.attack_obj)
            .insert(attack_transform);
    }
}

pub fn check_for_collision_with_attack_object(
    mut commands: Commands,
    mut collision_events: EventReader<bevy_rapier2d::prelude::CollisionEvent>,
    foreign_character_query: Query<(Entity, &mut ForeignCharacter, &Transform, &Velocity)>,
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
                        .find(|(foreign_character_entity, _, _, _)| {
                            *foreign_character_entity == *entity
                                || *foreign_character_entity == *entity1
                        });

                if let (
                    Some((_attack_ent, attack_object)),
                    Some((
                        foreign_entity,
                        mut foreign_character,
                        foreign_char_transform,
                        foreign_char_velocity,
                    )),
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

                    colliding_entity_commands
                        .insert(Velocity {
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
