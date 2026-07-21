use bevy::input::InputSystems;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

/// A semantic player action, independent of its physical key binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameAction {
    TogglePlanPanel,
    TogglePause,
    Cancel,
    DebugKillSelected,
}

/// Maps semantic actions to their current keyboard bindings.
#[derive(Resource, Debug, Clone)]
pub struct ActionMap {
    bindings: HashMap<GameAction, Vec<KeyCode>>,
}

impl Default for ActionMap {
    fn default() -> Self {
        Self {
            bindings: HashMap::from([
                (GameAction::TogglePlanPanel, vec![KeyCode::KeyP]),
                (GameAction::TogglePause, vec![KeyCode::Space]),
                (GameAction::Cancel, vec![KeyCode::Escape]),
                (GameAction::DebugKillSelected, vec![KeyCode::KeyK]),
            ]),
        }
    }
}

/// Frame-local semantic input state consumed by gameplay and UI systems.
#[derive(Resource, Debug, Default)]
pub struct ActionState {
    just_pressed: HashSet<GameAction>,
}

impl ActionState {
    pub fn just_pressed(&self, action: GameAction) -> bool {
        self.just_pressed.contains(&action)
    }
}

pub struct InputActionsPlugin;

impl Plugin for InputActionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActionMap>()
            .init_resource::<ActionState>()
            .add_systems(PreUpdate, update_action_state.after(InputSystems));
    }
}

fn update_action_state(
    keyboard: Res<ButtonInput<KeyCode>>,
    bindings: Res<ActionMap>,
    mut actions: ResMut<ActionState>,
) {
    actions.just_pressed.clear();

    for (&action, keys) in &bindings.bindings {
        if keys.iter().any(|key| keyboard.just_pressed(*key)) {
            actions.just_pressed.insert(action);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn default_bindings_are_exposed_by_semantic_action() {
        let bindings = ActionMap::default();

        assert_eq!(
            bindings.bindings.get(&GameAction::TogglePlanPanel),
            Some(&vec![KeyCode::KeyP])
        );
        assert_eq!(
            bindings.bindings.get(&GameAction::TogglePause),
            Some(&vec![KeyCode::Space])
        );
        assert_eq!(
            bindings.bindings.get(&GameAction::Cancel),
            Some(&vec![KeyCode::Escape])
        );
    }

    #[test]
    fn physical_keys_are_translated_to_semantic_actions() {
        let mut world = World::new();
        let mut keyboard = ButtonInput::<KeyCode>::default();
        keyboard.press(KeyCode::Space);
        world.insert_resource(keyboard);
        world.insert_resource(ActionMap::default());
        world.insert_resource(ActionState::default());

        world.run_system_once(update_action_state).unwrap();

        let actions = world.resource::<ActionState>();
        assert!(actions.just_pressed(GameAction::TogglePause));
        assert!(!actions.just_pressed(GameAction::Cancel));
    }
}
