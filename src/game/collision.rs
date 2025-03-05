use bevy::ecs::system::Resource;
use bevy_rapier2d::prelude::{CollisionGroups, Group};

#[repr(u32)]
pub enum CollisionGroup {
    MapObject = 0b0001,
    ForeignCharacter = 0b0100,
    AttackObj = 0b1000,
}

#[derive(Resource, Clone)]
pub struct CollisionGroupSet {
    /// Collides with all
    pub map_object: CollisionGroups,
    /// Collides with everything except SelfCharacter
    pub player: CollisionGroups,
    /// Collides with MapObject & ForeignCharacter, not SelfCharacter
    pub attack_obj: CollisionGroups,
}

impl Default for CollisionGroupSet {
    fn default() -> Self {
        Self::new()
    }
}

impl CollisionGroupSet {
    pub fn new() -> Self {
        Self {
            map_object: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::MapObject as u32),
                Group::from_bits_truncate(0b1111),
            ),
            player: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::ForeignCharacter as u32),
                Group::from_bits_truncate(0b1111),
            ),
            attack_obj: CollisionGroups::new(
                Group::from_bits_truncate(CollisionGroup::AttackObj as u32),
                Group::from_bits_truncate(
                    CollisionGroup::MapObject as u32 | CollisionGroup::ForeignCharacter as u32,
                ),
            ),
        }
    }
}
