// src/services/game_service.rs - Service de jeu refactorisé en modules

// Réexport du module principal
pub use self::game_service_module::*;

// Le module principal est maintenant dans game_service/
#[path = "game_service/mod.rs"]
mod game_service_module;