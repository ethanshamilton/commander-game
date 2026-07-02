use bevy::prelude::*;
use bevy::ui::Checked;
use bevy::ui_widgets::{Button, Checkbox, checkbox_self_update, observe};

pub struct TextButtonConfig {
    pub label: String,
    pub width: Val,
    pub height: Val,
    pub bg_color: Color,
    pub text_color: Color,
    pub text_size: f32,
}

impl Default for TextButtonConfig {
    fn default() -> Self {
        Self {
            label: "Button".to_string(),
            width: Val::Px(150.0),
            height: Val::Px(65.0),
            bg_color: Color::srgb(0.15, 0.15, 0.15),
            text_color: Color::srgb(0.9, 0.9, 0.9),
            text_size: 20.0,
        }
    }
}

pub fn spawn_text_button<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    config: TextButtonConfig,
    extra_components: B,
) {
    parent
        .spawn((
            Button,
            Node {
                width: config.width,
                height: config.height,
                border: UiRect::all(Val::Px(5.0)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BorderColor::all(Color::BLACK),
            BackgroundColor(config.bg_color),
            extra_components,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(config.label),
                TextFont {
                    font_size: FontSize::Px(config.text_size),
                    ..default()
                },
                TextColor(config.text_color),
            ));
        });
}

pub struct ToggleConfig {
    pub label: String,
    pub checked: bool,
    pub width: Val,
    pub height: Val,
    pub bg_color: Color,
    pub text_color: Color,
    pub text_size: f32,
}

impl Default for ToggleConfig {
    fn default() -> Self {
        Self {
            label: "Toggle".to_string(),
            checked: false,
            width: Val::Px(180.0),
            height: Val::Px(50.0),
            bg_color: Color::srgb(0.15, 0.15, 0.15),
            text_color: Color::srgb(0.9, 0.9, 0.9),
            text_size: 20.0,
        }
    }
}

pub fn spawn_checkbox_toggle<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    config: ToggleConfig,
    extra_components: B,
) {
    let mut entity = parent.spawn((
        Checkbox,
        Node {
            width: config.width,
            height: config.height,
            border: UiRect::all(Val::Px(5.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BorderColor::all(Color::BLACK),
        BackgroundColor(config.bg_color),
        observe(checkbox_self_update),
        extra_components,
    ));

    if config.checked {
        entity.insert(Checked);
    }

    entity.with_children(|parent| {
        parent.spawn((
            Text::new(config.label),
            TextFont {
                font_size: FontSize::Px(config.text_size),
                ..default()
            },
            TextColor(config.text_color),
        ));
    });
}

pub struct ListRowConfig {
    pub label: String,
    pub width: Val,
    pub height: Val,
}

impl Default for ListRowConfig {
    fn default() -> Self {
        Self {
            label: "List Item".to_string(),
            width: Val::Percent(100.0),
            height: Val::Px(44.0),
        }
    }
}

pub fn spawn_list_row<B: Bundle>(
    parent: &mut ChildSpawnerCommands,
    config: ListRowConfig,
    extra_components: B,
) {
    parent
        .spawn((
            Button,
            Node {
                width: config.width,
                height: config.height,
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                justify_content: JustifyContent::FlexStart,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.12, 0.12, 0.12, 0.85)),
            extra_components,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(config.label),
                TextFont {
                    font_size: FontSize::Px(22.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.9)),
            ));
        });
}
