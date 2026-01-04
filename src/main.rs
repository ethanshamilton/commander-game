use bevy::prelude::*;
mod components;
use components::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, (setup, setup_ui))
        .add_systems(Update, button_system)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.insert_resource(ClearColor(Color::srgb(0.1, 0.12, 0.14)));
}

#[derive(Component)]
struct SpawnSoldierButton;

fn setup_ui(mut commands: Commands) {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        width: Val::Px(150.0),
                        height: Val::Px(65.0),
                        border: UiRect::all(Val::Px(5.0)),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    BorderColor::all(Color::BLACK),
                    BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
                    SpawnSoldierButton,
                ))
                .with_children(|parent| {
                    parent.spawn((
                        Text::new("Spawn Soldier"),
                        TextFont {
                            font_size: 20.0,
                            ..default()
                        },
                        TextColor(Color::srgb(0.9, 0.9, 0.9)),
                    ));
                });
        });
}

fn button_system(
    mut commands: Commands,
    mut interaction_query: Query<
        (&Interaction, &mut BackgroundColor),
        (Changed<Interaction>, With<SpawnSoldierButton>),
    >,
) {
    for (interaction, mut color) in &mut interaction_query {
        match *interaction {
            Interaction::Pressed => {
                *color = BackgroundColor(Color::srgb(0.35, 0.75, 0.35));
                spawn_soldier(&mut commands);
            }
            Interaction::Hovered => {
                *color = BackgroundColor(Color::srgb(0.25, 0.25, 0.25));
            }
            Interaction::None => {
                *color = BackgroundColor(Color::srgb(0.15, 0.15, 0.15));
            }
        }
    }
}

fn spawn_soldier(commands: &mut Commands) {
    commands.spawn((
        Soldier {
            rank: Rank::Private,
            role: Role::Rifleman,
        },
        Allegiance { side: Side::Blue },
        Health {
            current: 100,
            max: 100,
        },
        Fitness { speed: 10 },
        Inventory { items: vec![] },
    ));

    info!("Soldier spawned! Rank: Private, Role: Rifleman, Side: Blue");
}
