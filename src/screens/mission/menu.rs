use crate::input::{ActionState, GameAction};
use crate::ui::widgets::{ToggleConfig, spawn_checkbox_toggle};
use bevy::camera::visibility::Visibility;
use bevy::prelude::*;
use bevy::ui::Checked;
use bevy::ui_widgets::{ValueChange, observe};
use std::collections::HashMap;

#[derive(Resource)]
pub(super) struct MenuState {
    states: HashMap<MenuId, bool>,
}

impl MenuState {
    fn new() -> Self {
        Self {
            states: HashMap::from([
                (MenuId::Meta, true),
                (MenuId::Plan, false),
                (MenuId::Settings, false),
            ]),
        }
    }

    fn is_open(&self, id: MenuId) -> bool {
        *self.states.get(&id).unwrap_or(&false)
    }

    fn set(&mut self, id: MenuId, is_open: bool) {
        self.states.insert(id, is_open);
    }
}

#[derive(Component)]
pub(super) struct Menu {
    pub id: MenuId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum MenuId {
    Meta,
    Plan,
    Settings,
}

#[derive(Component, Clone, Copy)]
struct MenuToggle {
    id: MenuId,
}

pub(super) fn register(app: &mut App) {
    app.insert_resource(MenuState::new()).add_systems(
        Update,
        (
            update_menu_visibility.after(toggle_plan_menu),
            toggle_plan_menu,
        )
            .run_if(in_state(crate::GameState::MissionScreen)),
    );
}

fn handle_menu_toggle_change(
    value_change: On<ValueChange<bool>>,
    mut menu_state: ResMut<MenuState>,
    toggles: Query<&MenuToggle>,
) {
    if let Ok(toggle) = toggles.get(value_change.source) {
        menu_state.set(toggle.id, value_change.value);
    }
}

fn toggle_plan_menu(
    actions: Res<ActionState>,
    mut commands: Commands,
    mut menu_state: ResMut<MenuState>,
    toggles: Query<(Entity, &MenuToggle, Has<Checked>)>,
) {
    if !actions.just_pressed(GameAction::TogglePlanPanel) {
        return;
    }

    let is_open = !menu_state.is_open(MenuId::Plan);
    menu_state.set(MenuId::Plan, is_open);

    for (entity, toggle, is_checked) in &toggles {
        if toggle.id != MenuId::Plan || is_checked == is_open {
            continue;
        }

        if is_open {
            commands.entity(entity).insert(Checked);
        } else {
            commands.entity(entity).remove::<Checked>();
        }
    }
}

fn update_menu_visibility(menu_state: Res<MenuState>, mut query: Query<(&Menu, &mut Visibility)>) {
    if !menu_state.is_changed() {
        return;
    }

    for (menu, mut visibility) in &mut query {
        *visibility = if menu_state.is_open(menu.id) {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

pub(super) fn spawn_sidebar(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Menu { id: MenuId::Meta },
            Node {
                width: Val::Px(200.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.1)),
        ))
        .with_children(|sidebar| {
            spawn_checkbox_toggle(
                sidebar,
                ToggleConfig {
                    label: "P".to_string(),
                    checked: false,
                    width: Val::Px(180.0),
                    height: Val::Px(50.0),
                    ..default()
                },
                (
                    MenuToggle { id: MenuId::Plan },
                    observe(handle_menu_toggle_change),
                ),
            );

            spawn_checkbox_toggle(
                sidebar,
                ToggleConfig {
                    label: "S".to_string(),
                    checked: false,
                    width: Val::Px(180.0),
                    height: Val::Px(50.0),
                    ..default()
                },
                (
                    MenuToggle {
                        id: MenuId::Settings,
                    },
                    observe(handle_menu_toggle_change),
                ),
            );
        });
}
