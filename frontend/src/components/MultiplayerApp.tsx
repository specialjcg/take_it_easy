// src/components/MultiplayerApp.tsx - Version refactorisée et modulaire
import { Component, createEffect, onMount, Show, createMemo, createSignal } from 'solid-js';
import { SessionState } from '../generated/common';
import { GameMode } from './GameModeSelector';

// Import des hooks personnalisés
import { useGameState } from '../hooks/useGameState';
import { useGameActions } from '../hooks/useGameActions';
import { usePolling } from '../hooks/usePolling';

// Import des services
import { GameStateManager } from '../services/GameStateManager';

// Import des composants UI
import { ConnectionInterface } from './ui/ConnectionInterface';
import { PlayersList } from './ui/PlayersList';
import { StatusMessages } from './ui/StatusMessages';
import { MCTSInterface } from './ui/MCTSInterface';
import { HexagonalGameBoard } from './ui/HexagonalGameBoard'; // ⚠️ IMPORT CORRIGÉ

// Import du CSS externe
import '../styles/multiplayer.css';

interface MultiplayerAppProps {
  gameMode: GameMode;
  autoConnectSolo: boolean;
  onBackToModeSelection: () => void;
}

/**
 * Composant principal refactorisé - Orchestrateur principal
 * Réduit de 2208 → ~150 lignes grâce à la modularisation
 */
const MultiplayerApp: Component<MultiplayerAppProps> = (props) => {
    // ============================================================================
    // HOOKS PERSONNALISÉS
    // ============================================================================

    const gameState = useGameState();

    // Détecter le mode MCTS viewer depuis l'URL
    const [isMctsViewer, setIsMctsViewer] = createSignal(false);

    const updatePlateauFunction = () => {
        const currentSession = gameState.session();
        const isViewer = currentSession && currentSession.playerId.includes('viewer');
        const isMctsMode = isMctsViewer();

        // FORCE: Détecter le mode mcts_view depuis l'URL directement
        const urlParams = new URLSearchParams(window.location.search);
        const isUrlMctsView = urlParams.get('mode') === 'mcts_view';

        console.log('🔍 DEBUG updatePlateauFunction:', {
            currentSession: currentSession?.playerId,
            isViewer,
            isMctsMode,
            isUrlMctsView,
            willUseViewer: isViewer || isMctsMode || isUrlMctsView
        });

        // Mode viewer : inclure les viewers normaux ET le mode mcts_view
        if (isViewer || isMctsMode || isUrlMctsView) {
            console.log('👁️ UTILISATION FONCTION VIEWER');
            // Mode viewer : afficher le plateau MCTS
            return (state: any) => GameStateManager.updatePlateauTilesForViewer(
                state,
                gameState.setPlateauTiles,
                gameState.setAvailablePositions,
                gameState.session,
            );
        } else {
            console.log('🎮 UTILISATION FONCTION NORMALE');
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
        gameState.loadingManager,
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
        gameState.setFinalScores,
        updatePlateauFunction(), // ✅ Fonction adaptée
        GameStateManager.convertSessionState,
    );

    // ============================================================================
    // EFFETS ET LIFECYCLE
    // ============================================================================

    // Auto-connexion via URL pour mode viewer seulement
    onMount(() => {
        // Détecter si on est en mode mcts_view
        const urlParams = new URLSearchParams(window.location.search);
        const mode = urlParams.get('mode');
        if (mode === 'mcts_view') {
            setIsMctsViewer(true);
            console.log('🔍 Mode MCTS Viewer détecté depuis URL');
        }

        // DEBUG: Log de la session du viewer
        console.log('🔍 DEBUG onMount:', {
            urlParams: Object.fromEntries(urlParams),
            isMctsViewer: mode === 'mcts_view'
        });

        // Seule l'auto-connexion via paramètres URL est conservée (mode viewer)
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

    // Auto-connexion en mode solo
    createEffect(() => {
        if (props.autoConnectSolo && !gameState.session()) {
            console.log('🤖 Auto-connexion mode solo déclenchée');

            // Générer un nom de joueur par défaut
            const defaultPlayerName = `Joueur-${Math.random().toString(36).substring(2, 6)}`;
            gameState.setPlayerName(defaultPlayerName);

            // Créer automatiquement une session avec le mode sélectionné
            setTimeout(() => {
                console.log('🎮 Création automatique session solo...');
                handleCreateSession();
            }, 500); // Petit délai pour s'assurer que les états sont bien initialisés
        }
    });

    // Auto-démarrage du premier tour en mode solo
    createEffect(() => {
        const state = gameState.gameState();
        const currentSession = gameState.session();

        // En mode solo, démarrer automatiquement le premier tour
        if (props.autoConnectSolo &&
            currentSession &&
            state &&
            state.state === SessionState.IN_PROGRESS &&
            gameState.currentTurnNumber() === 0 &&
            !gameState.currentTile()) {

            console.log('🎲 Auto-démarrage du premier tour en mode solo...');
            setTimeout(() => {
                handleStartGameTurn();
            }, 1000); // Délai pour laisser le temps au backend de s'initialiser
        }
    });


    // Démarrer le polling quand on rejoint une session
    createEffect(() => {
        const currentSession = gameState.session();
        if (currentSession) {
            console.log('🔍 DEBUG session connectée:', {
                sessionId: currentSession.sessionId,
                playerId: currentSession.playerId,
                isViewer: currentSession.playerId.includes('viewer'),
                isMctsMode: isMctsViewer()
            });
            polling.startPolling(currentSession.sessionId);
        } else {
            polling.stopPolling();
        }
    });

    // ✅ AUTO-DÉMARRAGE POUR LE MODE VIEWER MCTS
    createEffect(() => {
        const currentSession = gameState.session();
        const state = gameState.gameState();

        // Si on est en mode viewer ET qu'on a des données de plateau, marquer le jeu comme démarré
        if (currentSession &&
            (currentSession.playerId.includes('viewer') || currentSession.playerId.includes('mcts_viewer')) &&
            state &&
            !gameState.isGameStarted()) {

            console.log('👁️ VIEWER: Activation automatique du mode visualisation');
            gameState.setIsGameStarted(true);
            gameState.setStatusMessage('👁️ Mode visualisation MCTS activé');
        }
    });

    // Démarrer le jeu quand tous sont prêts
    createEffect(() => {
        const state = gameState.gameState();
        const currentSession = gameState.session();

        // ✅ NE PAS DÉCLENCHER SI ON EST EN MODE VIEWER
        const isViewer = currentSession && (currentSession.playerId.includes('viewer') || currentSession.playerId.includes('mcts_viewer'));

        if (state && state.state === SessionState.IN_PROGRESS && !gameState.isGameStarted() && !isViewer) {
            console.log('🎮 Jeu commencé ! Prêt pour démarrer le premier tour...');
            const currentPlayerScore = state.players?.find(p => p.id === currentSession?.playerId)?.score || 0;
            gameState.setStatusMessage(`🎯 Votre score actuel: ${currentPlayerScore} points`);
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
            GameStateManager.convertSessionState,
            props.gameMode.id
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
        GameStateManager.resetCache(); // ✅ AJOUTER CETTE LIGNE
        gameActions.leaveSession(gameState.resetSession);
    };

    const handleOpenMctsSession = () => {
        GameStateManager.openMctsSession(gameState.session);
    };

    const handleStartGameTurn = () => {
        gameActions.startGameTurn();
    };

    // ✅ CALCULER LE TITRE EN FONCTION DU MODE SÉLECTIONNÉ
    const gameTitle = createMemo(() => {
        return `${props.gameMode.icon} ${props.gameMode.name}`;
    });

    // ✅ MEMO STABLE POUR ÉVITER RE-CRÉATION DU COMPOSANT BOARD
    const stableBoardProps = createMemo(() => {
        const plateauData = gameState.plateauTiles();
        const positionsData = gameState.availablePositions();
        const sessionData = gameState.session();
        
        // Hash pour stabilité
        const hash = JSON.stringify({
            plateaus: plateauData,
            positions: positionsData,
            sessionId: sessionData?.playerId
        });
        
        
        return {
            plateauTiles: () => plateauData,
            availablePositions: () => positionsData,
            session: () => sessionData,
            hash
        };
    });

    const handlePlayMove = (position: number) => {
        const timestamp = performance.now();
        console.log(`🎯 [${timestamp.toFixed(0)}ms] handlePlayMove DÉBUT - position: ${position}`);

        // ✅ FONCTION OPTIMISTE POUR RÉACTIVITÉ IMMÉDIATE
        const updatePlateauTilesOptimistic = (pos: number, tile: string | null) => {
            if (tile) {
                GameStateManager.updatePlateauTilesOptimistic(
                    pos,
                    tile,
                    gameState.plateauTiles,
                    gameState.setPlateauTiles,
                    gameState.session,
                    gameState.currentTileImage() || undefined
                );
            }
        };

        console.log(`🚀 [${timestamp.toFixed(0)}ms] Appel gameActions.playMove...`);

        const startPlayMove = performance.now();
        gameActions.playMove(
            position,
            gameState.myTurn,
            polling.markActionPerformed,
            updatePlateauTilesOptimistic,
            gameState.currentTile
        );
        const endPlayMove = performance.now();

        console.log(`⏱️ [${endPlayMove.toFixed(0)}ms] gameActions.playMove terminé - durée: ${(endPlayMove - startPlayMove).toFixed(1)}ms`);
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
                    <div class="player-score-display">
                        <h3>🎯 Votre Score</h3>
                        <div class="current-score">
                            {(() => {
                                const currentSession = gameState.session();
                                const currentPlayer = state.players?.find(p => p.id === currentSession?.playerId);
                                return currentPlayer?.score || 0;
                            })()} points
                        </div>
                        
                        <div class="ready-section">
                            <Show when={!gameState.isPlayerReady()}>
                                <button
                                    onClick={handleSetReady}
                                    disabled={gameState.loadingManager.isAnyLoading()}
                                    class="ready-button"
                                >
                                    ✅ Je suis prêt !
                                </button>
                            </Show>
                            <Show when={gameState.isPlayerReady()}>
                                <div class="ready-status">
                                    <p>✅ Vous êtes prêt ! En attente des autres joueurs...</p>
                                </div>
                            </Show>
                        </div>
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
                                        disabled={gameState.loadingManager.isAnyLoading()}
                                        class="draw-tile-button"
                                    >
                                        🎲 Démarrer la partie
                                    </button>
                                </div>
                            </Show>


                            {/* Message d'attente simplifié */}
                            <Show when={gameState.isGameStarted() && gameState.currentTile() && !gameState.myTurn()}>
                                <div class="waiting-indicator">
                                    <span class="waiting-text">⏳ En attente des autres joueurs...</span>
                                </div>
                            </Show>
                        </div>

                        {/* 🔧 PLATEAU HEXAGONAL COMPLET AVEC PROPS STABLES */}
                        <HexagonalGameBoard
                            plateauTiles={stableBoardProps().plateauTiles}
                            availablePositions={stableBoardProps().availablePositions}
                            myTurn={gameState.myTurn}
                            session={stableBoardProps().session}
                            onTileClick={handlePlayMove}
                        />
                    </div>
                </Show>

                <Show when={state.state === SessionState.FINISHED}>
                    <div class="game-finished">
                        <h2>🎉 Partie terminée !</h2>
                        <div class="final-scores">
                            <h3>🏆 Classement final</h3>
                            {(() => {
                                const currentSession = gameState.session();
                                const players = gameState.gameState()?.players;
                                let finalList =
                                    players && players.length
                                        ? [...players]
                                        : (() => {
                                              const scores = gameState.finalScores();
                                              if (!scores) return [];
                                              return Object.entries(scores).map(([id, score]) => ({
                                                  id,
                                                  name: id === 'mcts_ai' ? '🤖 IA' : `Joueur ${id.slice(0, 4)}`,
                                                  score,
                                              }));
                                          })();

                                finalList.sort((a, b) => (b.score ?? 0) - (a.score ?? 0));

                                return finalList.length ? (
                                    <div class="score-list">
                                        {finalList.map((player) => (
                                            <div
                                                class={`score-item ${
                                                    player.id === currentSession?.playerId
                                                        ? 'player-score-self'
                                                        : ''
                                                } ${player.id === 'mcts_ai' ? 'player-score-ai' : ''}`}
                                            >
                                                <span class="player-name">
                                                    {player.id === 'mcts_ai' ? '🤖 IA' : player.name}
                                                </span>
                                                <span class="player-score">
                                                    {player.score ?? 0} points
                                                </span>
                                            </div>
                                        ))}
                                    </div>
                                ) : (
                                    <p>Aucun score disponible.</p>
                                );
                            })()}
                        </div>
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
            {/* Interface MCTS spécialisée - Pour MCTS réel ou viewer */}
            <Show when={gameState.session()?.playerId === 'mcts_ai' || isMctsViewer()}>
                <MCTSInterface
                    sessionCode={() => gameState.session()?.sessionCode || ''}
                    myTurn={gameState.myTurn}
                    renderGameBoard={renderGameBoard}
                />
            </Show>

            {/* Interface normale pour les joueurs humains */}
            <Show when={gameState.session() && gameState.session()?.playerId !== 'mcts_ai' && !isMctsViewer()}>
                <div class="header-section">
                    <div class="title-with-back">
                        <button
                            class="back-button"
                            onClick={props.onBackToModeSelection}
                            title="Retour à la sélection de mode"
                        >
                            ← Retour
                        </button>
                        <h1>{gameTitle()}</h1>
                    </div>
                    <p class="mode-description">{props.gameMode.description}</p>
                    <Show when={props.gameMode.simulations}>
                        <p class="mode-tech-info">🧠 MCTS : {props.gameMode.simulations} simulations par coup</p>
                    </Show>
                </div>



                {/* Messages d'état */}
                <StatusMessages
                    error={gameState.error}
                    statusMessage={gameState.statusMessage}
                />

                {/* Interface de connexion - Masquée en mode solo auto-connect */}
                <Show when={!gameState.session() && !props.autoConnectSolo}>
                    <ConnectionInterface
                        playerName={gameState.playerName}
                        setPlayerName={gameState.setPlayerName}
                        sessionCode={gameState.sessionCode}
                        setSessionCode={gameState.setSessionCode}
                        loading={gameState.loadingManager.isAnyLoading}
                        onCreateSession={handleCreateSession}
                        onJoinSession={handleJoinSession}
                    />
                </Show>

                {/* Message de chargement en mode solo */}
                <Show when={!gameState.session() && props.autoConnectSolo}>
                    <div class="loading-solo glass-container">
                        <h3>🤖 Préparation de la partie solo...</h3>
                        <p>Connexion automatique en cours...</p>
                        <div class="loading-spinner">⚡</div>
                    </div>
                </Show>

                {/* Interface de jeu */}
                <Show when={gameState.session()}>
                    <div class="session-info glass-container">
                        <div class="session-details">
                            <h2>🎮 Session: {gameState.session()?.sessionCode}</h2>
                            <p>Joueur: <strong>{gameState.playerName()}</strong></p>
                            <p class="player-id">ID: {gameState.session()?.playerId}</p>
                        </div>
                        <div class="session-actions">
                            {/* Tuile courante compacte */}
                            <Show when={gameState.currentTile() && gameState.currentTileImage()}>
                                <div class="compact-tile-display">
                                    <img 
                                        class="compact-tile-image" 
                                        src={gameState.currentTileImage() || ''}
                                        alt={`Tuile ${gameState.currentTile()}`}
                                    />
                                </div>
                            </Show>
                            <div style={{ display: 'flex', gap: '8px' }}>
                                <button
                                    class="open-mcts-button"
                                    onClick={handleOpenMctsSession}
                                    disabled={!gameState.session()}
                                >
                                    🤖 Voir session MCTS
                                </button>
                                <button onClick={handleLeaveSession} class="leave-button">
                                    Quitter la session
                                </button>
                            </div>
                        </div>
                    </div>

                    {renderGameBoard()}
                </Show>
            </Show>
        </div>
    );
};

export default MultiplayerApp;
