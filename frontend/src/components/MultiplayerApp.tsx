// src/components/MultiplayerApp.tsx - Version refactorisée et modulaire
import { Component, createEffect, onMount, Show } from 'solid-js';
import { SessionState } from '../generated/common';

// Import des hooks personnalisés
import { useGameState } from '../hooks/useGameState';
import { useGameActions } from '../hooks/useGameActions';
import { usePolling } from '../hooks/usePolling';

// Import des services
import { GameStateManager } from '../services/GameStateManager';

// Import des composants UI
import { ConnectionInterface } from './ui/ConnectionInterface';
import { PlayersList } from './ui/PlayersList';
import { CurrentTileDisplay } from './ui/CurrentTileDisplay';
import { StatusMessages } from './ui/StatusMessages';
import { MCTSInterface } from './ui/MCTSInterface';
import { HexagonalGameBoard } from './ui/HexagonalGameBoard'; // ⚠️ IMPORT CORRIGÉ

// Import du CSS externe
import '../styles/multiplayer.css';

/**
 * Composant principal refactorisé - Orchestrateur principal
 * Réduit de 2208 → ~150 lignes grâce à la modularisation
 */
const MultiplayerApp: Component = () => {
    // ============================================================================
    // HOOKS PERSONNALISÉS
    // ============================================================================

    const gameState = useGameState();
    const updatePlateauFunction = () => {
        const currentSession = gameState.session();
        if (currentSession && currentSession.playerId.includes('viewer')) {
            // Mode viewer : afficher tous les plateaux
            return (state: any) => GameStateManager.updatePlateauTilesForViewer(
                state,
                gameState.setPlateauTiles,
                gameState.setAvailablePositions,
                gameState.session,
            );
        } else {
            // Mode normal : afficher le plateau du joueur
            return (state: any) => GameStateManager.updatePlateauTiles(
                state,
                gameState.setPlateauTiles,
                gameState.setAvailablePositions,
                gameState.session,
            );
        }
    };
    const gameActions = useGameActions(
        gameState.session,
        gameState.setLoading,
        gameState.setError,
        gameState.setStatusMessage,
        gameState.setCurrentTile,
        gameState.setCurrentTileImage,
        gameState.setCurrentTurnNumber,
        gameState.setIsGameStarted,
        gameState.setMyTurn,
        gameState.setMctsLastMove,
        updatePlateauFunction(), // ✅ Fonction adaptée
    );

    const polling = usePolling(
        gameState.session,
        gameState.isGameStarted,
        gameState.currentTile,
        gameState.setGameState,
        gameState.setCurrentTile,
        gameState.setCurrentTileImage,
        gameState.setMyTurn,
        gameState.setIsGameStarted,
        gameState.setStatusMessage,
        updatePlateauFunction(), // ✅ Fonction adaptée
        GameStateManager.convertSessionState,
    );

    // ============================================================================
    // EFFETS ET LIFECYCLE
    // ============================================================================

    // Auto-connexion via URL
    onMount(() => {
        GameStateManager.handleAutoConnection(
            gameState.setPlayerName,
            gameState.setSessionCode,
            () => gameActions.joinSession(
                gameState.playerName,
                gameState.sessionCode,
                gameState.setSession,
                gameState.setGameState,
                GameStateManager.convertSessionState
            )
        );
    });

    // Démarrer le polling quand on rejoint une session
    createEffect(() => {
        const currentSession = gameState.session();
        if (currentSession) {
            polling.startPolling(currentSession.sessionId);
        } else {
            polling.stopPolling();
        }
    });

    // Démarrer le jeu quand tous sont prêts
    createEffect(() => {
        const state = gameState.gameState();
        if (state && state.state === SessionState.IN_PROGRESS && !gameState.isGameStarted()) {
            console.log('🎮 Jeu commencé ! Prêt pour démarrer le premier tour...');
            gameState.setStatusMessage('🎮 Le jeu commence ! Cliquez sur "Démarrer le tour"');
        }
    });

    // Gestion du cache d'images
    createEffect(() => {
        GameStateManager.updateImageCache(
            gameState.currentTile,
            gameState.currentTileImage,
            gameState.lastTileHash,
            gameState.setImageCache,
            gameState.setLastTileHash,
        );
    });

    // ============================================================================
    // HANDLERS D'ACTIONS
    // ============================================================================

    const handleCreateSession = () => {
        gameActions.createSession(
            gameState.playerName,
            gameState.setSession,
            gameState.setGameState,
            GameStateManager.convertSessionState
        );
    };

    const handleJoinSession = () => {
        gameActions.joinSession(
            gameState.playerName,
            gameState.sessionCode,
            gameState.setSession,
            gameState.setGameState,
            GameStateManager.convertSessionState
        );
    };

    const handleSetReady = () => {
        gameActions.setReady(gameState.setGameState);
    };

    const handleLeaveSession = () => {
        gameActions.leaveSession(gameState.resetSession);
    };

    const handleOpenMctsSession = () => {
        GameStateManager.openMctsSession(gameState.session);
    };

    const handleStartGameTurn = () => {
        gameActions.startGameTurn();
    };

    const handlePlayMove = (position: number) => {
        gameActions.playMove(position, gameState.myTurn);
    };

    // ============================================================================
    // RENDU DU PLATEAU DE JEU (CORRIGÉ AVEC HEXAGONES)
    // ============================================================================

    const renderGameBoard = () => {
        const state = gameState.gameState();
        if (!state) return null;

        return (
            <div class="game-board-section glass-container">
                <h3>🎮 Plateau de Jeu Take It Easy</h3>

                <div class="game-status">
                    <strong>État: {gameState.getSessionStateLabel(state.state)}</strong>
                    <Show when={gameState.isGameStarted()}>
                        <span class="current-turn">Tour: {gameState.currentTurnNumber()}/19</span>
                    </Show>
                </div>

                <Show when={state.state === SessionState.WAITING}>
                    <div class="waiting-message">
                        <p>⏳ En attente que tous les joueurs soient prêts...</p>
                    </div>
                </Show>

                <Show when={state.state === SessionState.IN_PROGRESS}>
                    <div class="classic-game-container">
                        <div class="classic-game-info">
                            {/* Bouton démarrer le tour */}
                            <Show when={!gameState.currentTile() && gameState.currentTurnNumber() === 0}>
                                <div class="draw-tile-section">
                                    <button
                                        onClick={handleStartGameTurn}
                                        disabled={gameState.loading()}
                                        class="draw-tile-button"
                                    >
                                        🎲 Démarrer la partie
                                    </button>
                                </div>
                            </Show>

                            {/* Affichage tuile courante */}
                            <CurrentTileDisplay
                                currentTile={gameState.currentTile}
                                currentTileImage={gameState.currentTileImage}
                                imageCache={gameState.imageCache}
                            />

                            {/* Status du tour */}
                            <Show when={gameState.isGameStarted() && gameState.currentTile()}>
                                <div class="turn-status">
                                    <Show when={gameState.myTurn()}>
                                        <div class="player-turn-indicator">
                                            <span class="turn-text">🎯 À votre tour !</span>
                                            <span class="positions-count">
                                                {gameState.availablePositions().length} positions disponibles
                                            </span>
                                        </div>
                                    </Show>
                                    <Show when={!gameState.myTurn()}>
                                        <div class="waiting-indicator">
                                            <span class="waiting-text">⏳ En attente des autres joueurs...</span>
                                        </div>
                                    </Show>
                                </div>
                            </Show>
                        </div>

                        {/* 🔧 PLATEAU HEXAGONAL COMPLET - REMPLACEMENT DU CANVAS VIDE */}
                        <HexagonalGameBoard
                            plateauTiles={gameState.plateauTiles}
                            availablePositions={gameState.availablePositions}
                            myTurn={gameState.myTurn}
                            session={gameState.session}
                            onTileClick={handlePlayMove}
                        />
                    </div>
                </Show>

                <Show when={state.state === SessionState.FINISHED}>
                    <div class="game-finished">
                        <h2>🎉 Partie terminée !</h2>
                        <PlayersList
                            gameState={gameState.gameState}
                            isCurrentPlayer={gameState.isCurrentPlayer}
                            getPlayerStatus={gameState.getPlayerStatus}
                            isPlayerReady={gameState.isPlayerReady}
                            loading={gameState.loading}
                            onSetReady={handleSetReady}
                            onOpenMctsSession={handleOpenMctsSession}
                            session={gameState.session}
                        />
                    </div>
                </Show>
            </div>
        );
    };

    // ============================================================================
    // RENDU PRINCIPAL
    // ============================================================================

    return (
        <div class="multiplayer-app">
            {/* Interface MCTS spécialisée */}
            <Show when={gameState.session()?.playerId === 'mcts_ai'}>
                <MCTSInterface
                    sessionCode={() => gameState.session()?.sessionCode || ''}
                    myTurn={gameState.myTurn}
                    renderGameBoard={renderGameBoard}
                />
            </Show>

            {/* Interface normale pour les joueurs humains */}
            <Show when={!gameState.session() || gameState.session()?.playerId !== 'mcts_ai'}>
                <h1>🎮 Take It Easy - Multiplayer vs MCTS</h1>



                {/* Messages d'état */}
                <StatusMessages
                    error={gameState.error}
                    statusMessage={gameState.statusMessage}
                />

                {/* Interface de connexion */}
                <Show when={!gameState.session()}>
                    <ConnectionInterface
                        playerName={gameState.playerName}
                        setPlayerName={gameState.setPlayerName}
                        sessionCode={gameState.sessionCode}
                        setSessionCode={gameState.setSessionCode}
                        loading={gameState.loading}
                        onCreateSession={handleCreateSession}
                        onJoinSession={handleJoinSession}
                    />
                </Show>

                {/* Interface de jeu */}
                <Show when={gameState.session()}>
                    <div class="session-info glass-container">
                        <div class="session-details">
                            <h2>🎮 Session: {gameState.session()?.sessionCode}</h2>
                            <p>Joueur: <strong>{gameState.playerName()}</strong></p>
                            <p class="player-id">ID: {gameState.session()?.playerId}</p>
                        </div>
                        <button onClick={handleLeaveSession} class="leave-button">
                            Quitter la session
                        </button>
                    </div>

                    <PlayersList
                        gameState={gameState.gameState}
                        isCurrentPlayer={gameState.isCurrentPlayer}
                        getPlayerStatus={gameState.getPlayerStatus}
                        isPlayerReady={gameState.isPlayerReady}
                        loading={gameState.loading}
                        onSetReady={handleSetReady}
                        onOpenMctsSession={handleOpenMctsSession}
                        session={gameState.session}
                    />

                    {renderGameBoard()}
                </Show>
            </Show>
        </div>
    );
};

export default MultiplayerApp;