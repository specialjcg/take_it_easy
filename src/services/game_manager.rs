// src/services/game_manager.rs - Intégration avec votre système existant

use std::collections::HashMap;
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};
use crate::generated::takeiteasygame::v1::*;

// Import de vos modules existants
use crate::game::create_deck::{create_deck, Deck};
use crate::game::plateau::{create_plateau_empty, Plateau};
use crate::game::tile::Tile;
use crate::game::plateau_is_full::is_plateau_full;
use crate::game::remove_tile_from_deck::replace_tile_in_deck;
use crate::mcts::algorithm::mcts_find_best_position_for_tile_with_nn;
use crate::neural::policy_value_net::{PolicyNet, ValueNet};
use crate::scoring::scoring::result;
use rand::Rng;
use crate::game::get_legal_moves::get_legal_moves;
// ============================================================================
// ADAPTATION DE VOS TYPES EXISTANTS VERS LE SYSTÈME FONCTIONNEL
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeItEasyGameState {
    pub session_id: String,
    pub deck: Deck,
    pub player_plateaus: HashMap<String, Plateau>, // player_id -> plateau
    pub current_tile: Option<Tile>,
    pub current_turn: usize,
    pub total_turns: usize,
    pub game_status: GameStatus,
    pub scores: HashMap<String, i32>,
    pub waiting_for_players: Vec<String>, // Qui doit encore jouer ce tour
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerMove {
    pub player_id: String,
    pub position: usize, // Position sur le plateau (0-46)
    pub tile: Tile,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MctsMove {
    pub position: usize,
    pub tile: Tile,
    pub evaluation_score: f32,
    pub search_depth: usize,
    pub variations_considered: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveResult {
    pub new_game_state: TakeItEasyGameState,
    pub points_earned: i32,
    pub mcts_response: Option<MctsMove>,
    pub is_game_over: bool,
    pub turn_completed: bool, // Si tous les joueurs ont joué ce tour
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameStatus {
    WaitingForPlayers,
    InProgress,
    Finished,
}

// ============================================================================
// FONCTIONS PURES - ADAPTATION DE VOTRE LOGIQUE EXISTANTE
// ============================================================================

pub fn create_take_it_easy_game(
    session_id: String,
    player_ids: Vec<String>
) -> TakeItEasyGameState {
    let deck = create_deck();
    let mut player_plateaus = HashMap::new();

    // Créer un plateau vide pour chaque joueur (y compris MCTS)
    for player_id in &player_ids {
        player_plateaus.insert(player_id.clone(), create_plateau_empty());
    }

    // Ajouter MCTS comme joueur automatique si pas déjà présent
    if !player_ids.contains(&"mcts_ai".to_string()) {
        player_plateaus.insert("mcts_ai".to_string(), create_plateau_empty());
    }

    TakeItEasyGameState {
        session_id,
        deck,
        player_plateaus,
        current_tile: None,
        current_turn: 0,
        total_turns: 19, // Comme dans votre implémentation
        game_status: GameStatus::InProgress,
        scores: player_ids.iter().map(|id| (id.clone(), 0)).collect(),
        waiting_for_players: vec![],
    }
}

// ============================================================================
// NOUVELLE LOGIQUE : Proposer une tuile seulement si tous ont fini le tour précédent
// ============================================================================

pub fn start_new_turn(
    mut game_state: TakeItEasyGameState
) -> Result<TakeItEasyGameState, String> {
    if game_state.current_turn >= game_state.total_turns {
        return Err("GAME_ALREADY_FINISHED".to_string());
    }

    // 🔧 UTILISER VOS FONCTIONS : Filtrer les tuiles valides comme dans simulate_game
    let valid_tiles: Vec<Tile> = game_state.deck
        .tiles
        .iter()
        .cloned()
        .filter(|tile| *tile != Tile(0, 0, 0))  // ✅ Même logique que simulate_game.rs
        .collect();

    if valid_tiles.is_empty() {
        return Err("NO_TILES_REMAINING".to_string());
    }

    // 🎲 Piocher une tuile aléatoire SEULEMENT parmi les tuiles valides
    let _tile_index = rand::rng().random_range(0..valid_tiles.len());
    let chosen_tile = valid_tiles[_tile_index];
    
    log::info!("🎲 Tuile tirée: {:?} (tour {})", chosen_tile, game_state.current_turn);

    // 🔧 UTILISER VOTRE FONCTION : Remplacer la tuile dans le deck
    game_state.deck = replace_tile_in_deck(&game_state.deck, &chosen_tile);
    game_state.current_tile = Some(chosen_tile);

    // 🔧 TOUS LES JOUEURS (humains + MCTS) peuvent jouer immédiatement
    game_state.waiting_for_players = game_state.player_plateaus.keys().cloned().collect();

    Ok(game_state)
}

// Fonction utilitaire pour vérifier si on peut proposer une nouvelle tuile
// Dans game_manager.rs - NOUVELLE fonction utilisant vos concepts




// Dans game_manager.rs - AMÉLIORER ensure_current_tile
pub fn ensure_current_tile(mut game_state: TakeItEasyGameState) -> Result<TakeItEasyGameState, String> {
    if game_state.current_tile.is_some() {
        // ✅ Une tuile existe déjà, pas besoin de modification
        return Ok(game_state);
    }

    // 🎲 Aucune tuile courante, en tirer une NOUVELLE
    game_state = start_new_turn(game_state)?;

    Ok(game_state)
}
// game_manager.rs - dans apply_player_move
// Dans game_manager.rs - AMÉLIORER apply_player_move
pub fn apply_player_move(
    mut game_state: TakeItEasyGameState,
    player_move: PlayerMove
) -> Result<TakeItEasyGameState, String> {

    // Vérifications utilisant vos fonctions
    if game_state.current_tile.is_none() {
        return Err("NO_CURRENT_TILE".to_string());
    }

    let current_tile = game_state.current_tile.unwrap();

    if player_move.tile != current_tile {
        return Err("WRONG_TILE".to_string());
    }

    // 🔧 UTILISER VOS FONCTIONS : Vérifier les mouvements légaux
    let player_plateau = game_state.player_plateaus.get(&player_move.player_id)
        .ok_or_else(|| {
            "PLAYER_NOT_FOUND".to_string()
        })?;

    let legal_moves = get_legal_moves(player_plateau.clone());
    if !legal_moves.contains(&player_move.position) {
        return Err("ILLEGAL_MOVE".to_string());
    }

    // Récupérer le plateau du joueur pour modification
    let player_plateau = game_state.player_plateaus.get_mut(&player_move.player_id)
        .ok_or_else(|| "PLAYER_NOT_FOUND".to_string())?;

    // Placer la tuile
    player_plateau.tiles[player_move.position] = player_move.tile;

    // Retirer le joueur de la liste d'attente
    game_state.waiting_for_players.retain(|id| id != &player_move.player_id);


    Ok(game_state)
}

// Dans game_manager.rs - AMÉLIORER process_mcts_turn avec vos fonctions
pub async fn process_mcts_turn(
    mut game_state: TakeItEasyGameState,
    policy_net: &Mutex<PolicyNet>,
    value_net: &Mutex<ValueNet>,
    num_simulations: usize
) -> Result<(TakeItEasyGameState, MctsMove), String> {
    let current_tile = game_state.current_tile.ok_or("NO_CURRENT_TILE")?;

    // ✅ VÉRIFICATION: MCTS ne peut jouer que s'il est en attente
    if !game_state.waiting_for_players.contains(&"mcts_ai".to_string()) {
        return Err("MCTS_NOT_WAITING".to_string());
    }

    // Récupérer le plateau MCTS
    let mcts_plateau = game_state.player_plateaus.get_mut("mcts_ai")
        .ok_or("MCTS_PLAYER_NOT_FOUND")?;

    // ✅ VÉRIFICATION: Mouvements légaux
    let legal_moves = get_legal_moves(mcts_plateau.clone());
    if legal_moves.is_empty() {
        return Err("NO_LEGAL_MOVES_FOR_MCTS".to_string());
    }
    let mut deck_clone = game_state.deck.clone();

    // Utiliser MCTS pour choisir la position
    let policy_locked = policy_net.lock().await;
    let value_locked = value_net.lock().await;

    let mcts_result = mcts_find_best_position_for_tile_with_nn(
        mcts_plateau,
        &mut deck_clone,
        current_tile,
        &*policy_locked,
        &*value_locked,
        num_simulations,
        game_state.current_turn,
        game_state.total_turns,
    );

    // ✅ VALIDATION: Position légale
    if !legal_moves.contains(&mcts_result.best_position) {
        log::error!("❌ MCTS a choisi un mouvement illégal: {} (légaux: {:?})",
            mcts_result.best_position, legal_moves);
        return Err("MCTS_ILLEGAL_MOVE".to_string());
    }

    // ✅ PLACEMENT UNIQUE DE LA TUILE
    mcts_plateau.tiles[mcts_result.best_position] = current_tile;

    // ✅ RETIRER MCTS DE LA LISTE D'ATTENTE (important !)
    game_state.waiting_for_players.retain(|id| id != "mcts_ai");

    let mcts_move = MctsMove {
        position: mcts_result.best_position,
        tile: current_tile,
        evaluation_score: mcts_result.subscore as f32,
        search_depth: num_simulations,
        variations_considered: num_simulations,
    };
    Ok((game_state, mcts_move))
}

// Dans game_manager.rs - NOUVELLE fonction de debug complète
// ============================================================================
// NOUVELLE LOGIQUE : Séparer fin de tour et proposition de tuile
// ============================================================================

pub fn check_turn_completion(
    mut game_state: TakeItEasyGameState
) -> Result<TakeItEasyGameState, String> {
    // Si tous les joueurs (humains + MCTS) ont joué
    if game_state.waiting_for_players.is_empty() {
        let _completed_turn = game_state.current_turn;
        game_state.current_turn += 1;
        game_state.current_tile = None;
        
        // Mettre à jour les scores après chaque tour
        for (player_id, plateau) in &game_state.player_plateaus {
            let current_score = result(plateau);
            game_state.scores.insert(player_id.clone(), current_score);
        }
        
        // Vérifier si le jeu est terminé
        log::info!("🔍 Tour {}/{}, plateaux pleins: {}", 
                  game_state.current_turn, 
                  game_state.total_turns,
                  game_state.player_plateaus.values().all(|p| is_plateau_full(p)));
        
        if game_state.current_turn >= game_state.total_turns {
            game_state.game_status = GameStatus::Finished;
            log::info!("🏁 Jeu terminé par tours! Scores finaux: {:?}", game_state.scores);
        } else if game_state.player_plateaus.values().all(|plateau| is_plateau_full(plateau)) {
            game_state.game_status = GameStatus::Finished;
            log::info!("🏁 Jeu terminé par plateaux pleins! Scores finaux: {:?}", game_state.scores);
        } else {
            // ✅ RETOUR À L'AUTOMATISME : Démarrer immédiatement le tour suivant
            game_state = start_new_turn(game_state)?;
        }
    }

    Ok(game_state)
}


pub fn calculate_final_scores(game_state: &TakeItEasyGameState) -> HashMap<String, i32> {
    let mut scores = HashMap::new();

    // Utiliser votre fonction de scoring existante
    for (player_id, plateau) in &game_state.player_plateaus {
        let score = result(plateau);
        scores.insert(player_id.clone(), score);
    }

    scores
}

pub fn is_game_finished(game_state: &TakeItEasyGameState) -> bool {
    matches!(game_state.game_status, GameStatus::Finished) ||
        game_state.current_turn >= game_state.total_turns ||
        game_state.player_plateaus.values().all(|plateau| is_plateau_full(plateau))
}

pub fn get_available_positions(game_state: &TakeItEasyGameState, player_id: &str) -> Vec<usize> {
    if let Some(plateau) = game_state.player_plateaus.get(player_id) {
        plateau.tiles.iter()
            .enumerate()
            .filter(|(_, tile)| **tile == Tile(0, 0, 0))
            .map(|(index, _)| index)
            .collect()
    } else {
        vec![]
    }
}

// ============================================================================
// FONCTIONS D'ÉTAT DES JOUEURS - POUR FLOW INDÉPENDANT
// ============================================================================

pub fn get_player_status(game_state: &TakeItEasyGameState, player_id: &str) -> PlayerStatus {
    if is_game_finished(game_state) {
        PlayerStatus::GameFinished
    } else if game_state.current_tile.is_none() {
        // Pas de tuile = en attente d'une nouvelle tuile
        PlayerStatus::WaitingForNewTile
    } else if game_state.waiting_for_players.contains(&player_id.to_string()) {
        // Ce joueur peut jouer la tuile actuelle
        PlayerStatus::CanPlay
    } else {
        // Ce joueur a déjà joué, attend les autres
        PlayerStatus::WaitingForOthers
    }
}

pub fn get_all_players_status(game_state: &TakeItEasyGameState) -> HashMap<String, PlayerStatus> {
    let mut status_map = HashMap::new();
    
    for player_id in game_state.player_plateaus.keys() {
        status_map.insert(player_id.clone(), get_player_status(game_state, player_id));
    }
    
    status_map
}





#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerStatus {
    CanPlay,              // Joueur peut jouer (tuile disponible, à son tour)
    WaitingForOthers,     // Joueur a joué, attend que les autres finissent
    WaitingForNewTile,    // Pas de tuile courante, attend le prochain tour
    GameFinished,         // Jeu terminé
}

// ============================================================================
// FONCTIONS DE COMPOSITION - LOGIQUE MÉTIER COMPLÈTE
// ============================================================================

// game_manager.rs - votre fonction reste la même, mais on change la logique
pub async fn process_player_move_with_mcts(
    game_state: TakeItEasyGameState,
    player_move: PlayerMove,
    policy_net: &Mutex<PolicyNet>,
    value_net: &Mutex<ValueNet>,
    num_simulations: usize
) -> Result<MoveResult, String> {
    // 1. Appliquer le mouvement du joueur
    let mut new_state = apply_player_move(game_state, player_move.clone())?;

    // 2. ✅ GESTION UNIQUE DE MCTS ICI
    let mcts_response = if player_move.player_id != "mcts_ai"
        && new_state.waiting_for_players.contains(&"mcts_ai".to_string()) {
        // MCTS joue automatiquement UNE SEULE FOIS
        match process_mcts_turn(new_state.clone(), policy_net, value_net, num_simulations).await {
            Ok((updated_state, mcts_move)) => {
                new_state = updated_state;                Some(mcts_move)
            },
            Err(_e) => {
                new_state.waiting_for_players.retain(|id| id != "mcts_ai");
                None
            }
        }
    } else {
        None
    };

    // 3. Vérifier la fin du tour (démarre automatiquement le tour suivant)
    new_state = check_turn_completion(new_state)?;

    // 4. Calculer et mettre à jour les scores en temps réel
    for (player_id, plateau) in &new_state.player_plateaus {
        let current_score = result(plateau);
        new_state.scores.insert(player_id.clone(), current_score);
    }
    
    let initial_score = new_state.scores.get(&player_move.player_id).unwrap_or(&0).clone();
    let points_earned = if let Some(plateau) = new_state.player_plateaus.get(&player_move.player_id) {
        result(plateau) - initial_score
    } else {
        0
    };

    Ok(MoveResult {
        new_game_state: new_state.clone(),
        points_earned,
        mcts_response,
        is_game_over: is_game_finished(&new_state),
        turn_completed: new_state.waiting_for_players.is_empty(),
    })
}
// ============================================================================
// CONVERSION VERS PROTOBUF (COMPATIBLE AVEC VOS TYPES)
// ============================================================================

pub fn take_it_easy_state_to_protobuf(state: &TakeItEasyGameState, game_mode: &str) -> GameState {
    let players: Vec<crate::generated::takeiteasygame::v1::Player> = state.scores.iter().map(|(player_id, score)| {
        crate::generated::takeiteasygame::v1::Player {
            id: player_id.clone(),
            name: player_id.clone(),
            score: *score,
            is_ready: true,
            is_connected: true,
            joined_at: chrono::Utc::now().timestamp(),
        }
    }).collect();

    GameState {
        session_id: state.session_id.clone(),
        players,
        current_player_id: state.waiting_for_players.first().cloned().unwrap_or_default(),
        state: match state.game_status {
            GameStatus::WaitingForPlayers => 0,
            GameStatus::InProgress => 1,
            GameStatus::Finished => 2,
        },
        board_state: serde_json::to_string(state).unwrap_or_default(),
        turn_number: state.current_turn as i32,
        game_mode: game_mode.to_string(),
    }
}

pub fn player_move_from_json(move_data: &str, player_id: &str) -> Result<PlayerMove, String> {
    #[derive(Deserialize)]
    struct MoveData {
        position: usize,
        #[allow(dead_code)]
        tile: Option<(i32, i32, i32)>, // Optionnel car défini par le serveur
    }

    let data: MoveData = serde_json::from_str(move_data)
        .map_err(|e| format!("Invalid move format: {}", e))?;

    Ok(PlayerMove {
        player_id: player_id.to_string(),
        position: data.position,
        tile: Tile(0, 0, 0), // Sera remplacé par la tuile courante
        timestamp: chrono::Utc::now().timestamp(),
    })
}

pub fn mcts_move_to_json(mcts_move: &MctsMove) -> Result<String, String> {
    serde_json::to_string(mcts_move)
        .map_err(|e| format!("Failed to serialize MCTS move: {}", e))
}