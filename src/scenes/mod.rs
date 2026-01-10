pub mod main_menu;
pub mod mission;
pub mod settings;

// Re-export commonly used items from each scene
pub use mission::{
    cleanup_mission_scene, setup_sidebar, setup_unit_bar, spawn_soldier, update_menu_visibility,
    Menu, MenuId, MenuState, MissionScreenRoot,
};
