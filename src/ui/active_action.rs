use bevy::prelude::*;

/// Shared description of the interaction the player is currently performing.
/// Any gameplay/UI system may set or clear it; the view is intentionally
/// decoupled from plan placement.
#[derive(Resource, Debug, Default, Clone, PartialEq, Eq)]
pub struct ActiveAction {
    label: Option<String>,
}

impl ActiveAction {
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn set(&mut self, label: impl Into<String>) {
        let label = label.into();
        if self.label.as_deref() != Some(label.as_str()) {
            self.label = Some(label);
        }
    }

    pub fn clear(&mut self) {
        if self.label.is_some() {
            self.label = None;
        }
    }
}

/// Marker for the reusable indicator's panel node.
#[derive(Component)]
pub struct ActiveActionPanel;

/// Marker for the text child inside [`ActiveActionPanel`].
#[derive(Component)]
pub struct ActiveActionText;

pub struct ActiveActionPlugin;

impl Plugin for ActiveActionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ActiveAction>()
            .add_systems(Update, render_active_action);
    }
}

fn render_active_action(
    action: Res<ActiveAction>,
    mut panels: Query<&mut Node, With<ActiveActionPanel>>,
    mut texts: Query<&mut Text, With<ActiveActionText>>,
) {
    let label = action.label();
    for mut panel in &mut panels {
        panel.display = if label.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
    if let Some(label) = label {
        for mut text in &mut texts {
            **text = label.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_action_can_be_replaced_and_cleared() {
        let mut action = ActiveAction::default();
        assert_eq!(action.label(), None);
        action.set("Create Line Start");
        assert_eq!(action.label(), Some("Create Line Start"));
        action.set("Assign Plan: Select Squad Leader");
        assert_eq!(action.label(), Some("Assign Plan: Select Squad Leader"));
        action.clear();
        assert_eq!(action.label(), None);
    }
}
