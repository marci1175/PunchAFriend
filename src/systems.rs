use bevy::{
    asset::Assets,
    color::Color,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        component::Component,
        entity::{Entities, Entity},
        query::With,
        system::{Commands, Query, Res, ResMut},
        world::Mut,
    },
    input::{keyboard::KeyCode, ButtonInput},
    math::{primitives::Circle, Vec3},
    render::mesh::{Mesh, Mesh2d},
    sprite::{ColorMaterial, MeshMaterial2d},
    transform::components::Transform,
};

#[derive(Component, Clone)]
pub struct SelfCharacter;

pub fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    commands.spawn((
        Mesh2d(meshes.add(Circle::new(20.))),
        MeshMaterial2d(materials.add(ColorMaterial::from_color(Color::linear_rgb(200., 0., 0.)))),
        SelfCharacter,
        Transform::from_xyz(0., 0., 0.),
    ));
}

pub fn frame(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut entities: Query<Entity, With<SelfCharacter>>,
) {
    let e = entities.get_single().unwrap();

    let keyboard_input = keyboard_input.clone();

    commands
        .entity(e)
        .entry()
        .and_modify(move |mut transform: Mut<Transform>| {
            if keyboard_input.pressed(KeyCode::ArrowLeft) {
                transform.translation.x -= 1.0;
            }

            if keyboard_input.pressed(KeyCode::ArrowRight) {
                transform.translation.x += 1.0;
            }

            if keyboard_input.pressed(KeyCode::ArrowUp) {
                transform.translation.y += 1.0;
            }

            if keyboard_input.pressed(KeyCode::ArrowDown) {
                transform.translation.y -= 1.0;
            }
        });
}
