// AppMVU.tsx - Composant principal avec architecture MVU
// Design moderne et focus sur le plateau hexagonal
import { Component, Show, onMount, createEffect, onCleanup, createSignal } from 'solid-js';
import { getStore } from '../mvu';
import { SessionState } from '../generated/common';
import GameModeSelector from './GameModeSelector';
import type { GameMode } from '../mvu/model';

// Import des composants UI
import AuthPage from './ui/AuthPage';
import { ConnectionInterface } from './ui/ConnectionInterface';
import { StatusMessages } from './ui/StatusMessages';
import { MCTSInterface } from './ui/MCTSInterface';
import { HexagonalGameBoard } from './ui/HexagonalGameBoard';
import { RecordingStats } from './ui/RecordingStats';

// Import du CSS trendy
import '../styles/app-trendy.css';

const AppMVU: Component = () => {
    const store = getStore();

    // Vérifier l'authentification et détecter le mode viewer au démarrage
    onMount(() => {
        store.dispatch(store.msg.checkAuth());

        const urlParams = new URLSearchParams(window.location.search);
        const mode = urlParams.get('mode');

        if (mode === 'mcts_view' || mode === 'viewer') {
            console.log('Mode viewer détecté au démarrage');
            store.dispatch(store.msg.setMctsViewer(true));
            store.dispatch(store.msg.skipAuth());

            const sessionCode = urlParams.get('session');
            const playerName = urlParams.get('player') || 'MCTS_Viewer';
            if (sessionCode) {
                store.dispatch(store.msg.setPlayerName(playerName));
                store.dispatch(store.msg.setSessionCode(sessionCode));
                const viewerMode: GameMode = {
                    id: 'viewer-mode',
                    name: 'MCTS Viewer',
                    description: 'Mode observation des parties MCTS',
                    icon: '👁️',
                };
                store.dispatch(store.msg.navigateToGame(viewerMode, false));
                setTimeout(() => store.dispatch(store.msg.joinSession()), 100);
            }
        }
    });

    // Gérer le polling quand on a une session
    createEffect(() => {
        const session = store.session();
        if (session) {
            store.startPolling(session.sessionId);
        } else {
            store.stopPolling();
        }
    });

    // Auto-connexion en mode solo
    const [autoConnectTriggered, setAutoConnectTriggered] = createSignal(false);
    createEffect(() => {
        const shouldAutoConnect = store.autoConnectSolo() &&
                                  !store.session() &&
                                  store.currentView() === 'game' &&
                                  !store.model().loading.createSession &&
                                  !autoConnectTriggered();

        if (shouldAutoConnect) {
            setAutoConnectTriggered(true);
            const defaultPlayerName = `Joueur-${Math.random().toString(36).substring(2, 6)}`;
            store.dispatch(store.msg.setPlayerName(defaultPlayerName));
            setTimeout(() => store.dispatch(store.msg.createSession()), 500);
        }
    });

    createEffect(() => {
        if (store.currentView() !== 'game') {
            setAutoConnectTriggered(false);
        }
    });

    // Auto-ready en mode solo
    const [autoReadyTriggered, setAutoReadyTriggered] = createSignal(false);
    createEffect(() => {
        const state = store.gameState();
        const session = store.session();

        if (
            store.autoConnectSolo() &&
            session &&
            state &&
            state.state === SessionState.WAITING &&
            !store.isPlayerReady() &&
            !autoReadyTriggered() &&
            !store.model().loading.setReady
        ) {
            setAutoReadyTriggered(true);
            setTimeout(() => store.dispatch(store.msg.setReady()), 500);
        }
    });

    createEffect(() => {
        if (!store.session()) {
            setAutoReadyTriggered(false);
        }
    });

    // Auto-démarrage du premier tour en mode solo
    createEffect(() => {
        const state = store.gameState();
        const session = store.session();
        const model = store.model();

        if (
            store.autoConnectSolo() &&
            session &&
            state &&
            state.state === SessionState.IN_PROGRESS &&
            store.currentTurnNumber() === 0 &&
            !store.currentTile() &&
            !model.hasAutoStarted
        ) {
            store.dispatch(store.msg.markAutoStarted());
            setTimeout(() => store.dispatch(store.msg.startTurn()), 1000);
        }
    });

    onCleanup(() => {
        store.stopPolling();
    });

    // Handlers
    const handleModeSelected = (mode: GameMode) => {
        const autoConnect = mode.id.startsWith('single-player') || mode.id === 'training';
        store.dispatch(store.msg.navigateToGame(mode, autoConnect));
    };

    const handleBackToModeSelection = () => {
        store.stopPolling();
        store.dispatch(store.msg.navigateToModeSelection());
    };

    const handlePlayMove = (position: number) => {
        store.dispatch(store.msg.playMove(position));
    };

    // Render game board avec design amélioré
    const renderGameBoard = () => {
        return (
            <Show when={store.gameState()} keyed>
                {(state) => (
                    <div class="game-board-section glass-container">
                        <h3>Plateau de Jeu Take It Easy</h3>

                        <div class="game-status">
                            <strong>État: {store.getSessionStateLabel(store.gameState()?.state ?? 0)}</strong>
                            <Show when={store.isGameStarted()}>
                                <span class="current-turn">Tour: {store.currentTurnNumber()}/19</span>
                            </Show>
                        </div>

                        <Show when={store.gameState()?.state === SessionState.WAITING}>
                            <div class="player-score-display">
                                <h3>Votre Score</h3>
                                <div class="current-score">
                                    {store.gameState()?.players?.find(p => p.id === store.session()?.playerId)?.score || 0} points
                                </div>
                                <div class="ready-section">
                                    <Show when={!store.isPlayerReady()}>
                                        <button
                                            onClick={() => store.dispatch(store.msg.setReady())}
                                            disabled={store.isLoading()}
                                            class="ready-button"
                                        >
                                            Je suis prêt !
                                        </button>
                                    </Show>
                                    <Show when={store.isPlayerReady()}>
                                        <div class="ready-status">
                                            <p>Vous êtes prêt ! En attente des autres joueurs...</p>
                                        </div>
                                    </Show>
                                </div>
                            </div>
                        </Show>

                        <Show when={store.gameState()?.state === SessionState.IN_PROGRESS}>
                            {/* Scores en temps réel */}
                            <div class="live-scores">
                                {store.gameState()?.players
                                    ?.filter(p => !p.id.includes('viewer'))
                                    .map(player => (
                                        <div class={`live-score-item ${player.id === store.session()?.playerId ? 'self' : ''} ${player.id === 'mcts_ai' ? 'ai' : ''}`}>
                                            <span class="live-score-name">
                                                {player.id === 'mcts_ai' ? '🤖 IA' : `👤 ${player.name}`}
                                            </span>
                                            <span class="live-score-value">{player.score} pts</span>
                                        </div>
                                    ))
                                }
                            </div>

                            <div class="classic-game-container">
                                <div class="classic-game-info">
                                    <Show when={!store.currentTile() && store.currentTurnNumber() === 0}>
                                        <div class="draw-tile-section">
                                            <button
                                                onClick={() => store.dispatch(store.msg.startTurn())}
                                                disabled={store.isLoading()}
                                                class="draw-tile-button"
                                            >
                                                Démarrer la partie
                                            </button>
                                        </div>
                                    </Show>

                                    <Show when={store.isGameStarted() && store.currentTile() && !store.myTurn()}>
                                        <div class="waiting-indicator">
                                            <span class="waiting-text">En attente des autres joueurs...</span>
                                        </div>
                                    </Show>
                                </div>

                                <HexagonalGameBoard
                                    plateauTiles={store.plateauTiles}
                                    availablePositions={store.availablePositions}
                                    myTurn={store.myTurn}
                                    session={store.session}
                                    onTileClick={handlePlayMove}
                                    currentTile={store.currentTile}
                                    isGameStarted={store.isGameStarted}
                                />
                            </div>
                        </Show>

                        <Show when={store.gameState()?.state === SessionState.FINISHED}>
                            <div class="game-finished">
                                <h2>Partie terminée !</h2>

                                {/* Scores finaux */}
                                <div class="final-scores">
                                    <h3>Classement final</h3>
                                    {renderFinalScores()}
                                </div>

                                {/* Affichage des deux plateaux */}
                                <div class="final-boards">
                                    <div class="final-board-container">
                                        <h4>👤 Votre plateau</h4>
                                        <HexagonalGameBoard
                                            plateauTiles={store.plateauTiles}
                                            availablePositions={() => []}
                                            myTurn={() => false}
                                            session={store.session}
                                            onTileClick={() => {}}
                                            displayPlayerId={store.session()?.playerId}
                                            readOnly={true}
                                            size="small"
                                        />
                                        <div class="final-board-score">
                                            {store.gameState()?.players?.find(p => p.id === store.session()?.playerId)?.score || 0} pts
                                        </div>
                                    </div>

                                    <div class="final-board-container">
                                        <h4>🤖 Plateau IA</h4>
                                        <HexagonalGameBoard
                                            plateauTiles={store.plateauTiles}
                                            availablePositions={() => []}
                                            myTurn={() => false}
                                            session={store.session}
                                            onTileClick={() => {}}
                                            displayPlayerId="mcts_ai"
                                            readOnly={true}
                                            size="small"
                                        />
                                        <div class="final-board-score ai">
                                            {store.gameState()?.players?.find(p => p.id === 'mcts_ai')?.score || 0} pts
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </Show>
                    </div>
                )}
            </Show>
        );
    };

    const renderFinalScores = () => {
        const state = store.gameState();
        const session = store.session();
        const scores = store.finalScores();

        let finalList = state?.players?.length
            ? [...state.players]
            : scores
            ? Object.entries(scores).map(([id, score]) => ({
                  id,
                  name: id === 'mcts_ai' ? 'IA' : `Joueur ${id.slice(0, 4)}`,
                  score,
              }))
            : [];

        // Filtrer les viewers et scores inutiles
        finalList = finalList.filter(player =>
            !player.id.includes('viewer') &&
            !(player.id !== 'mcts_ai' && player.id !== session?.playerId && player.score === 0)
        );

        finalList.sort((a, b) => (b.score ?? 0) - (a.score ?? 0));
        const maxScore = finalList.length > 0 ? finalList[0].score : 0;

        return finalList.length ? (
            <div class="score-list">
                {finalList.map((player, index) => {
                    const isWinner = (player.score ?? 0) === maxScore;
                    return (
                        <div
                            class={`score-item ${
                                player.id === session?.playerId ? 'player-score-self' : ''
                            } ${player.id === 'mcts_ai' ? 'player-score-ai' : ''} ${
                                isWinner ? 'winner' : ''
                            }`}
                        >
                            <span class="player-name">
                                {isWinner && index === 0 ? '🏆 ' : ''}
                                {player.id === 'mcts_ai' ? '🤖 IA' : `👤 ${player.name}`}
                            </span>
                            <span class="player-score">{player.score ?? 0} pts</span>
                        </div>
                    );
                })}
            </div>
        ) : (
            <p>Aucun score disponible.</p>
        );
    };

    // Main render
    return (
        <div class="app-container">
            {/* Login/Register Page */}
            <Show when={store.currentView() === 'login'}>
                <AuthPage />
            </Show>

            {/* Mode Selection */}
            <Show when={store.currentView() === 'mode-selection'}>
                <div class="user-header">
                    <Show when={store.isAuthenticated()}>
                        <div class="user-info">
                            <span class="user-name">
                                Connecté: <strong>{store.user()?.username}</strong>
                            </span>
                            <button
                                class="logout-button"
                                onClick={() => store.dispatch(store.msg.logout())}
                            >
                                Déconnexion
                            </button>
                        </div>
                    </Show>
                    <Show when={!store.isAuthenticated()}>
                        <div class="guest-info">
                            <span>Mode invité</span>
                            <button
                                class="login-link"
                                onClick={() => store.dispatch({ type: 'CHECK_AUTH_FAILURE' })}
                            >
                                Se connecter
                            </button>
                        </div>
                    </Show>
                </div>
                <GameModeSelector onModeSelected={handleModeSelected} />
            </Show>

            {/* Game View - MCTS Interface */}
            <Show when={store.currentView() === 'game' && store.isMctsViewer()}>
                <MCTSInterface
                    sessionCode={() => store.session()?.sessionCode || ''}
                    myTurn={store.myTurn}
                    renderGameBoard={renderGameBoard}
                />
            </Show>

            {/* Game View - Connection (no session yet) */}
            <Show when={store.currentView() === 'game' && !store.session() && !store.isMctsViewer()}>
                <div class="header-section">
                    <div class="title-with-back">
                        <button
                            class="back-button"
                            onClick={handleBackToModeSelection}
                            title="Retour à la sélection de mode"
                        >
                            ← Retour
                        </button>
                        <h1>
                            {store.selectedGameMode()?.icon} {store.selectedGameMode()?.name}
                        </h1>
                    </div>
                    <p class="mode-description">{store.selectedGameMode()?.description}</p>
                    <Show when={store.selectedGameMode()?.simulations}>
                        <p class="mode-tech-info">
                            MCTS : {store.selectedGameMode()?.simulations} simulations par coup
                        </p>
                    </Show>
                </div>

                <StatusMessages error={store.error} statusMessage={store.statusMessage} />

                <Show when={!store.autoConnectSolo()}>
                    <ConnectionInterface
                        playerName={store.playerName}
                        setPlayerName={(name) => store.dispatch(store.msg.setPlayerName(name))}
                        sessionCode={store.sessionCode}
                        setSessionCode={(code) => store.dispatch(store.msg.setSessionCode(code))}
                        loading={store.isLoading}
                        onCreateSession={() => store.dispatch(store.msg.createSession())}
                        onJoinSession={() => store.dispatch(store.msg.joinSession())}
                    />
                </Show>

                <Show when={store.autoConnectSolo()}>
                    <div class="loading-solo glass-container">
                        <h3>Préparation de la partie solo...</h3>
                        <p>Connexion automatique en cours...</p>
                        <div class="loading-spinner"></div>
                    </div>
                </Show>
            </Show>

            {/* Game View - Playing (has session) */}
            <Show
                when={
                    store.currentView() === 'game' &&
                    store.session() &&
                    store.session()?.playerId !== 'mcts_ai' &&
                    !store.isMctsViewer()
                }
            >
                <div class="header-section">
                    <div class="title-with-back">
                        <button
                            class="back-button"
                            onClick={handleBackToModeSelection}
                            title="Retour à la sélection de mode"
                        >
                            ← Retour
                        </button>
                        <h1>
                            {store.selectedGameMode()?.icon} {store.selectedGameMode()?.name}
                        </h1>
                    </div>
                    <p class="mode-description">{store.selectedGameMode()?.description}</p>
                </div>

                {/* Status message avec info de tour */}
                <Show when={store.currentTile()}>
                    <div class="status-message">
                        Tour {store.currentTurnNumber()}: {store.currentTile()}
                    </div>
                </Show>

                <StatusMessages error={store.error} statusMessage={!store.currentTile() ? store.statusMessage : (() => '')} />

                {/* Session Info Card */}
                <div class="session-info glass-container">
                    <div class="session-details">
                        <h2>Session: {store.session()?.sessionCode}</h2>
                        <p>
                            Joueur: <strong>{store.playerName()}</strong>
                        </p>
                        <p class="player-id">ID: {store.session()?.playerId}</p>
                    </div>
                    <div class="session-actions">
                        <Show when={store.currentTile() && store.currentTileImage()}>
                            <div class="compact-tile-display">
                                <img
                                    class="compact-tile-image"
                                    src={store.currentTileImage() || ''}
                                    alt={`Tuile ${store.currentTile()}`}
                                />
                            </div>
                        </Show>
                        <div style={{ display: 'flex', gap: '8px' }}>
                            <button
                                class="open-mcts-button"
                                onClick={() => store.dispatch(store.msg.openMctsSession())}
                                disabled={!store.session()}
                            >
                                Voir session MCTS
                            </button>
                            <button
                                onClick={() => store.dispatch(store.msg.leaveSession())}
                                class="leave-button"
                            >
                                Quitter la session
                            </button>
                        </div>
                    </div>
                </div>

                {/* Recording Stats */}
                <RecordingStats />

                {/* Game Board - Focus principal */}
                {renderGameBoard()}
            </Show>
        </div>
    );
};

export default AppMVU;
