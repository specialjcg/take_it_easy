// mvu/effects.ts - Side effects (appels gRPC, etc.)
import { gameClient } from '../services/GameClient';
import type { Model, GameStateData, Session } from './model';
import type { Msg } from './messages';
import { SessionState } from '../generated/common';
import { authApi, saveAuth, clearAuth, loadAuth } from './auth';

// ============================================================================
// EFFECT TYPE - Représente un side effect à exécuter
// ============================================================================

export type Effect =
    | { type: 'NONE' }
    | { type: 'LOGIN'; email: string; password: string }
    | { type: 'REGISTER'; email: string; username: string; password: string }
    | { type: 'LOGOUT' }
    | { type: 'CHECK_AUTH' }
    | { type: 'CREATE_SESSION'; playerName: string; gameMode: string }
    | { type: 'JOIN_SESSION'; sessionCode: string; playerName: string }
    | { type: 'SET_READY'; sessionId: string; playerId: string }
    | { type: 'LEAVE_SESSION'; sessionId: string; playerId: string }
    | { type: 'START_TURN'; sessionId: string }
    | { type: 'PLAY_MOVE'; sessionId: string; playerId: string; position: number }
    | { type: 'POLL_STATE'; sessionId: string }
    | { type: 'OPEN_MCTS_WINDOW'; sessionCode: string }
    | { type: 'BATCH'; effects: Effect[] };

// ============================================================================
// EFFET EXTRACTION - Détermine quels effets exécuter après un message
// ============================================================================

export const getEffects = (model: Model, msg: Msg): Effect => {
    console.log('getEffects called:', msg.type);
    switch (msg.type) {
        // Auth effects
        case 'LOGIN_REQUEST':
            return { type: 'LOGIN', email: msg.email, password: msg.password };

        case 'REGISTER_REQUEST':
            return { type: 'REGISTER', email: msg.email, username: msg.username, password: msg.password };

        case 'LOGOUT_REQUEST':
            return { type: 'LOGOUT' };

        case 'CHECK_AUTH_REQUEST':
            return { type: 'CHECK_AUTH' };

        case 'CREATE_SESSION_REQUEST':
            // Note: la vérification loading est faite dans update(), ici on vérifie juste les données
            if (!model.playerName.trim()) {
                return { type: 'NONE' };
            }
            return {
                type: 'CREATE_SESSION',
                playerName: model.playerName,
                gameMode: model.selectedGameMode?.id || 'multiplayer',
            };

        case 'JOIN_SESSION_REQUEST':
            if (!model.playerName.trim() || !model.sessionCode.trim()) {
                return { type: 'NONE' };
            }
            return {
                type: 'JOIN_SESSION',
                sessionCode: model.sessionCode,
                playerName: model.playerName,
            };

        case 'SET_READY_REQUEST':
            if (!model.session) return { type: 'NONE' };
            return {
                type: 'SET_READY',
                sessionId: model.session.sessionId,
                playerId: model.session.playerId,
            };

        case 'LEAVE_SESSION_REQUEST':
            if (!model.session) return { type: 'NONE' };
            return {
                type: 'LEAVE_SESSION',
                sessionId: model.session.sessionId,
                playerId: model.session.playerId,
            };

        case 'START_TURN_REQUEST':
            if (!model.session) return { type: 'NONE' };
            return {
                type: 'START_TURN',
                sessionId: model.session.sessionId,
            };

        case 'PLAY_MOVE_REQUEST':
            if (!model.session || !model.myTurn) return { type: 'NONE' };
            return {
                type: 'PLAY_MOVE',
                sessionId: model.session.sessionId,
                playerId: model.session.playerId,
                position: msg.position,
            };

        case 'OPEN_MCTS_SESSION':
            if (!model.session) return { type: 'NONE' };
            return {
                type: 'OPEN_MCTS_WINDOW',
                sessionCode: model.session.sessionCode,
            };

        default:
            return { type: 'NONE' };
    }
};

// ============================================================================
// EFFECT RUNNERS - Exécute les effets et retourne des messages
// ============================================================================

export const runEffect = async (
    effect: Effect,
    dispatch: (msg: Msg) => void
): Promise<void> => {
    console.log('runEffect:', effect.type, effect);
    switch (effect.type) {
        case 'NONE':
            return;

        case 'LOGIN': {
            const result = await authApi.login(effect.email, effect.password);
            if (result.success && result.user && result.token) {
                saveAuth(result.token, result.user);
                dispatch({ type: 'LOGIN_SUCCESS', user: result.user, token: result.token });
            } else {
                dispatch({ type: 'LOGIN_FAILURE', error: result.error || 'Erreur de connexion' });
            }
            break;
        }

        case 'REGISTER': {
            const result = await authApi.register(effect.email, effect.username, effect.password);
            if (result.success && result.user && result.token) {
                saveAuth(result.token, result.user);
                dispatch({ type: 'REGISTER_SUCCESS', user: result.user, token: result.token });
            } else {
                dispatch({ type: 'REGISTER_FAILURE', error: result.error || 'Erreur d\'inscription' });
            }
            break;
        }

        case 'LOGOUT': {
            clearAuth();
            dispatch({ type: 'LOGOUT_COMPLETE' });
            break;
        }

        case 'CHECK_AUTH': {
            const { token, user } = loadAuth();
            if (token && user) {
                // Vérifier que le token est toujours valide
                const result = await authApi.getCurrentUser(token);
                if (result.success && result.user) {
                    dispatch({ type: 'CHECK_AUTH_SUCCESS', user: result.user, token });
                } else {
                    clearAuth();
                    dispatch({ type: 'CHECK_AUTH_FAILURE' });
                }
            } else {
                dispatch({ type: 'CHECK_AUTH_FAILURE' });
            }
            break;
        }

        case 'CREATE_SESSION': {
            console.log('Effect CREATE_SESSION:', effect.playerName, effect.gameMode);
            const result = await gameClient.createSession(effect.playerName, effect.gameMode);
            console.log('CREATE_SESSION result:', result);

            if (result.success && result.sessionId && result.playerId && result.sessionCode) {
                const session: Session = {
                    sessionId: result.sessionId,
                    playerId: result.playerId,
                    sessionCode: result.sessionCode,
                };

                const gameState: GameStateData = result.sessionState
                    ? convertSessionState(result.sessionState)
                    : {
                          sessionCode: result.sessionCode,
                          state: SessionState.WAITING,
                          players: [{
                              id: result.playerId,
                              name: effect.playerName,
                              score: 0,
                              isReady: true,
                              isConnected: true,
                              joinedAt: Date.now().toString(),
                          }],
                          boardState: '{}',
                      };

                dispatch({ type: 'CREATE_SESSION_SUCCESS', session, gameState });
            } else {
                dispatch({
                    type: 'CREATE_SESSION_FAILURE',
                    error: result.error || 'Erreur lors de la création',
                });
            }
            break;
        }

        case 'JOIN_SESSION': {
            const result = await gameClient.joinSession(effect.sessionCode, effect.playerName);

            if (result.success && result.sessionId && result.playerId && result.sessionCode) {
                const session: Session = {
                    sessionId: result.sessionId,
                    playerId: result.playerId,
                    sessionCode: result.sessionCode,
                };

                const gameState: GameStateData = result.sessionState
                    ? convertSessionState(result.sessionState)
                    : {
                          sessionCode: result.sessionCode,
                          state: SessionState.WAITING,
                          players: [{
                              id: result.playerId,
                              name: effect.playerName,
                              score: 0,
                              isReady: false,
                              isConnected: true,
                              joinedAt: Date.now().toString(),
                          }],
                          boardState: '{}',
                      };

                dispatch({ type: 'JOIN_SESSION_SUCCESS', session, gameState });
            } else {
                dispatch({
                    type: 'JOIN_SESSION_FAILURE',
                    error: result.error || 'Erreur lors du join',
                });
            }
            break;
        }

        case 'SET_READY': {
            console.log('SET_READY effect:', effect.sessionId, effect.playerId);
            const result = await gameClient.setPlayerReady(effect.sessionId, effect.playerId);
            console.log('SET_READY result:', result);

            if (result.success) {
                console.log('SET_READY success, gameStarted:', result.gameStarted);
                dispatch({ type: 'SET_READY_SUCCESS', gameStarted: result.gameStarted || false });
            } else {
                console.log('SET_READY failure:', result.error);
                dispatch({ type: 'SET_READY_FAILURE', error: result.error || 'Erreur' });
            }
            break;
        }

        case 'LEAVE_SESSION': {
            await gameClient.leaveSession(effect.sessionId, effect.playerId);
            dispatch({ type: 'LEAVE_SESSION_COMPLETE' });
            break;
        }

        case 'START_TURN': {
            const result = await gameClient.startNewTurn(effect.sessionId);

            if (result.success) {
                dispatch({
                    type: 'START_TURN_SUCCESS',
                    tile: result.announcedTile || '',
                    tileImage: result.tileImage || '',
                    turnNumber: result.turnNumber || 0,
                    waitingForPlayers: result.waitingForPlayers || [],
                    gameState: result.gameState,
                });

                // Extraire les plateaux du gameState retourné
                if (result.gameState) {
                    try {
                        const gameStateData = typeof result.gameState === 'string'
                            ? JSON.parse(result.gameState)
                            : result.gameState;

                        if (gameStateData.player_plateaus) {
                            const plateauTiles: Record<string, string[]> = {};
                            let availablePositions: number[] = [];

                            Object.entries(gameStateData.player_plateaus).forEach(([playerId, plateau]: [string, any]) => {
                                plateauTiles[playerId] = plateau.tile_images || [];
                                // Utiliser les positions du joueur humain, pas de l'IA
                                if (plateau.available_positions && playerId !== 'mcts_ai') {
                                    availablePositions = plateau.available_positions;
                                }
                            });

                            dispatch({
                                type: 'UPDATE_PLATEAU_TILES',
                                plateauTiles,
                                availablePositions,
                            });
                        }
                    } catch (e) {
                        console.warn('Failed to parse gameState in START_TURN:', e);
                    }
                }
            } else {
                dispatch({ type: 'START_TURN_FAILURE', error: result.error || 'Erreur tour' });
            }
            break;
        }

        case 'PLAY_MOVE': {
            const result = await gameClient.makeMove(
                effect.sessionId,
                effect.playerId,
                effect.position
            );

            if (result.success) {
                dispatch({
                    type: 'PLAY_MOVE_SUCCESS',
                    position: effect.position,
                    pointsEarned: result.pointsEarned || 0,
                    newGameState: result.newGameState || {},
                    mctsResponse: result.mctsResponse,
                    isGameOver: result.isGameOver || false,
                });

                // Extraire les plateaux du newGameState
                if (result.newGameState?.player_plateaus) {
                    const plateauTiles: Record<string, string[]> = {};
                    let availablePositions: number[] = [];

                    Object.entries(result.newGameState.player_plateaus).forEach(([playerId, plateau]: [string, any]) => {
                        plateauTiles[playerId] = plateau.tile_images || [];
                        // Utiliser les positions du joueur humain, pas de l'IA
                        if (plateau.available_positions && playerId !== 'mcts_ai') {
                            availablePositions = plateau.available_positions;
                        }
                    });

                    dispatch({
                        type: 'UPDATE_PLATEAU_TILES',
                        plateauTiles,
                        availablePositions,
                    });
                }

                // Déclencher le tour suivant après un délai
                if (!result.isGameOver && (result.newGameState?.turnNumber ?? 0) < 19) {
                    setTimeout(() => {
                        dispatch({ type: 'START_TURN_REQUEST' });
                    }, 2000);
                }
            } else {
                dispatch({ type: 'PLAY_MOVE_FAILURE', error: result.error || 'Mouvement refusé' });
            }
            break;
        }

        case 'POLL_STATE': {
            try {
                // D'abord récupérer l'état de session (toujours disponible)
                const sessionResult = await gameClient.getSessionState(effect.sessionId);
                if (sessionResult.success && sessionResult.sessionState) {
                    const gameState = convertSessionState(sessionResult.sessionState);

                    // Récupérer les plateaux si partie en cours (1) OU terminée (2)
                    // Pour avoir l'état final des plateaux à la fin de partie
                    if (gameState.state === 1 || gameState.state === 2) {
                        const gameResult = await gameClient.getGameState(effect.sessionId);

                        if (gameResult.success && gameResult.gameState) {
                            // Parser le gameState JSON pour extraire les plateaux
                            let gameStateData: any = {};
                            try {
                                gameStateData = typeof gameResult.gameState === 'string'
                                    ? JSON.parse(gameResult.gameState)
                                    : gameResult.gameState;
                            } catch (e) {
                                console.warn('Failed to parse gameState:', e);
                            }

                            // Extraire les plateaux et positions disponibles
                            if (gameStateData.player_plateaus) {
                                const plateauTiles: Record<string, string[]> = {};
                                let availablePositions: number[] = [];

                                Object.entries(gameStateData.player_plateaus).forEach(([playerId, plateau]: [string, any]) => {
                                    plateauTiles[playerId] = plateau.tile_images || [];
                                    // Utiliser les positions du joueur humain, pas de l'IA
                                    if (plateau.available_positions && playerId !== 'mcts_ai') {
                                        availablePositions = plateau.available_positions;
                                    }
                                });

                                dispatch({
                                    type: 'UPDATE_PLATEAU_TILES',
                                    plateauTiles,
                                    availablePositions,
                                });
                            }

                            dispatch({
                                type: 'POLL_STATE_SUCCESS',
                                gameState,
                                currentTile: gameResult.currentTile,
                                currentTileImage: gameResult.currentTileImage,
                                turnNumber: gameResult.currentTurn,
                                isGameOver: gameResult.isGameFinished,
                                finalScores: gameResult.finalScores ? JSON.parse(gameResult.finalScores) : undefined,
                            });
                        } else {
                            // getGameState a échoué mais on a quand même les infos session
                            dispatch({
                                type: 'POLL_STATE_SUCCESS',
                                gameState,
                            });
                        }
                    } else {
                        // Partie pas encore en cours (WAITING), juste mettre à jour l'état session
                        dispatch({
                            type: 'POLL_STATE_SUCCESS',
                            gameState,
                        });
                    }
                }
            } catch (error) {
                console.error('POLL_STATE error:', error);
                dispatch({ type: 'POLL_STATE_FAILURE', error: String(error) });
            }
            break;
        }

        case 'OPEN_MCTS_WINDOW': {
            const url = `${window.location.origin}${window.location.pathname}?mode=mcts_view&session=${effect.sessionCode}&player=MCTS_Viewer`;
            window.open(url, '_blank', 'width=800,height=900');
            break;
        }

        case 'BATCH': {
            await Promise.all(effect.effects.map(e => runEffect(e, dispatch)));
            break;
        }
    }
};

// ============================================================================
// HELPERS
// ============================================================================

const convertSessionState = (sessionState: any): GameStateData => {
    return {
        sessionCode: sessionState.sessionId || '',
        state: sessionState.state ?? SessionState.WAITING,
        players: (sessionState.players || []).map((p: any) => ({
            id: p.id || '',
            name: p.name || 'Joueur',
            score: p.score || 0,
            isReady: p.isReady || false,
            isConnected: p.isConnected ?? true,
            joinedAt: p.joinedAt?.toString() || Date.now().toString(),
        })),
        boardState: sessionState.boardState || '{}',
        currentTurn: sessionState.currentPlayerId,
        gameMode: sessionState.gameMode,
    };
};
