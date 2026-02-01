// mvu/update.ts - Fonction update pure (Model, Msg) -> Model
import { Model, initialModel } from './model';
import type { Msg } from './messages';

// ============================================================================
// UPDATE FUNCTION - Fonction pure qui retourne un nouveau Model
// ============================================================================

export const update = (model: Model, msg: Msg): Model => {
    console.log('MVU Update:', msg.type, msg);
    switch (msg.type) {
        // ====================================================================
        // AUTH
        // ====================================================================
        case 'LOGIN_REQUEST':
            return {
                ...model,
                auth: { ...model.auth, loginLoading: true, authError: '' },
            };

        case 'LOGIN_SUCCESS':
            return {
                ...model,
                auth: {
                    ...model.auth,
                    isAuthenticated: true,
                    user: msg.user,
                    token: msg.token,
                    loginLoading: false,
                    authError: '',
                },
                currentView: 'mode-selection',
                playerName: msg.user.username, // Utiliser le nom d'utilisateur
            };

        case 'LOGIN_FAILURE':
            return {
                ...model,
                auth: { ...model.auth, loginLoading: false, authError: msg.error },
            };

        case 'REGISTER_REQUEST':
            return {
                ...model,
                auth: { ...model.auth, registerLoading: true, authError: '' },
            };

        case 'REGISTER_SUCCESS':
            return {
                ...model,
                auth: {
                    ...model.auth,
                    isAuthenticated: true,
                    user: msg.user,
                    token: msg.token,
                    registerLoading: false,
                    authError: '',
                },
                currentView: 'mode-selection',
                playerName: msg.user.username,
            };

        case 'REGISTER_FAILURE':
            return {
                ...model,
                auth: { ...model.auth, registerLoading: false, authError: msg.error },
            };

        case 'LOGOUT_REQUEST':
            return model; // Effect handles logout

        case 'LOGOUT_COMPLETE':
            return {
                ...initialModel,
                currentView: 'login',
            };

        case 'CHECK_AUTH_REQUEST':
            return model;

        case 'CHECK_AUTH_SUCCESS':
            return {
                ...model,
                auth: {
                    ...model.auth,
                    isAuthenticated: true,
                    user: msg.user,
                    token: msg.token,
                },
                currentView: 'mode-selection',
                playerName: msg.user.username,
            };

        case 'CHECK_AUTH_FAILURE':
            return {
                ...model,
                auth: { ...model.auth, isAuthenticated: false, user: null, token: null },
                currentView: 'login',
            };

        case 'SWITCH_AUTH_VIEW':
            return {
                ...model,
                auth: { ...model.auth, authView: msg.view, authError: '' },
            };

        case 'CLEAR_AUTH_ERROR':
            return {
                ...model,
                auth: { ...model.auth, authError: '' },
            };

        case 'SKIP_AUTH':
            return {
                ...model,
                currentView: 'mode-selection',
            };

        // ====================================================================
        // NAVIGATION
        // ====================================================================
        case 'NAVIGATE_TO_GAME':
            return {
                ...model,
                currentView: 'game',
                selectedGameMode: msg.gameMode,
                autoConnectSolo: msg.autoConnect,
                error: '',
                statusMessage: '',
            };

        case 'NAVIGATE_TO_MODE_SELECTION':
            return {
                ...initialModel,
                currentView: 'mode-selection',
            };

        // ====================================================================
        // INPUT UTILISATEUR
        // ====================================================================
        case 'SET_PLAYER_NAME':
            return { ...model, playerName: msg.name };

        case 'SET_SESSION_CODE':
            return { ...model, sessionCode: msg.code };

        // ====================================================================
        // SESSION - REQUESTS (loading states)
        // ====================================================================
        case 'CREATE_SESSION_REQUEST':
            // Ignorer si déjà en cours ou si on a déjà une session
            if (model.loading.createSession || model.session) {
                return model;
            }
            return {
                ...model,
                loading: { ...model.loading, createSession: true },
                error: '',
            };

        case 'JOIN_SESSION_REQUEST':
            return {
                ...model,
                loading: { ...model.loading, joinSession: true },
                error: '',
            };

        case 'SET_READY_REQUEST':
            return {
                ...model,
                loading: { ...model.loading, setReady: true },
            };

        case 'LEAVE_SESSION_REQUEST':
            return model; // Pas de loading pour leave

        // ====================================================================
        // SESSION - RESPONSES
        // ====================================================================
        case 'CREATE_SESSION_SUCCESS':
            return {
                ...model,
                loading: { ...model.loading, createSession: false },
                session: msg.session,
                gameState: msg.gameState,
                statusMessage: `Session créée ! Code: ${msg.session.sessionCode}`,
            };

        case 'CREATE_SESSION_FAILURE':
            return {
                ...model,
                loading: { ...model.loading, createSession: false },
                error: msg.error,
            };

        case 'JOIN_SESSION_SUCCESS':
            return {
                ...model,
                loading: { ...model.loading, joinSession: false },
                session: msg.session,
                gameState: msg.gameState,
                statusMessage: `Rejoint la session ${msg.session.sessionCode}`,
            };

        case 'JOIN_SESSION_FAILURE':
            return {
                ...model,
                loading: { ...model.loading, joinSession: false },
                error: msg.error,
            };

        case 'SET_READY_SUCCESS': {
            if (!model.gameState || !model.session) {
                return { ...model, loading: { ...model.loading, setReady: false } };
            }

            const updatedPlayers = model.gameState.players.map(p =>
                p.id === model.session?.playerId ? { ...p, isReady: true } : p
            );

            return {
                ...model,
                loading: { ...model.loading, setReady: false },
                gameState: {
                    ...model.gameState,
                    players: updatedPlayers,
                    state: msg.gameStarted ? 1 : model.gameState.state,
                },
                statusMessage: msg.gameStarted ? 'La partie commence !' : 'Vous êtes maintenant prêt !',
            };
        }

        case 'SET_READY_FAILURE':
            return {
                ...model,
                loading: { ...model.loading, setReady: false },
                error: msg.error,
            };

        case 'LEAVE_SESSION_COMPLETE':
            return {
                ...initialModel,
                currentView: model.currentView,
                selectedGameMode: model.selectedGameMode,
            };

        // ====================================================================
        // GAMEPLAY - REQUESTS
        // ====================================================================
        case 'START_TURN_REQUEST':
            return {
                ...model,
                loading: { ...model.loading, startTurn: true },
                error: '',
            };

        case 'PLAY_MOVE_REQUEST':
            return {
                ...model,
                loading: { ...model.loading, playMove: true },
                myTurn: false, // Bloquer immédiatement les clics
                statusMessage: `Position ${msg.position}...`,
                error: '',
            };

        // ====================================================================
        // GAMEPLAY - RESPONSES
        // ====================================================================
        case 'START_TURN_SUCCESS':
            return {
                ...model,
                loading: { ...model.loading, startTurn: false },
                currentTile: msg.tile,
                currentTileImage: msg.tileImage,
                currentTurnNumber: msg.turnNumber,
                isGameStarted: true,
                myTurn: msg.waitingForPlayers.includes(model.session?.playerId || ''),
                statusMessage: `Tour ${msg.turnNumber}: ${msg.tile}`,
            };

        case 'START_TURN_FAILURE':
            return {
                ...model,
                loading: { ...model.loading, startTurn: false },
                error: msg.error,
            };

        case 'PLAY_MOVE_SUCCESS':
            return {
                ...model,
                loading: { ...model.loading, playMove: false },
                statusMessage: `Position ${msg.position}! +${msg.pointsEarned} pts`,
            };

        case 'PLAY_MOVE_FAILURE':
            return {
                ...model,
                loading: { ...model.loading, playMove: false },
                myTurn: true, // Rollback - rendre le tour
                error: msg.error,
                statusMessage: msg.error,
            };

        // ====================================================================
        // POLLING
        // ====================================================================
        case 'POLL_STATE_SUCCESS': {
            const updates: Partial<Model> = {
                gameState: msg.gameState,
                loading: { ...model.loading, polling: false },
            };

            if (msg.currentTile !== undefined) updates.currentTile = msg.currentTile;
            if (msg.currentTileImage !== undefined) updates.currentTileImage = msg.currentTileImage;
            if (msg.turnNumber !== undefined) updates.currentTurnNumber = msg.turnNumber;
            if (msg.isGameOver) updates.finalScores = msg.finalScores || null;

            return { ...model, ...updates };
        }

        case 'POLL_STATE_FAILURE':
            return {
                ...model,
                loading: { ...model.loading, polling: false },
            };

        // ====================================================================
        // UI
        // ====================================================================
        case 'CLEAR_ERROR':
            return { ...model, error: '' };

        case 'CLEAR_STATUS_MESSAGE':
            return { ...model, statusMessage: '' };

        case 'SET_STATUS_MESSAGE':
            return { ...model, statusMessage: msg.message };

        case 'SET_ERROR':
            return { ...model, error: msg.error };

        // ====================================================================
        // MCTS
        // ====================================================================
        case 'SET_MCTS_VIEWER_MODE':
            return { ...model, isMctsViewer: msg.enabled };

        case 'OPEN_MCTS_SESSION':
            // Side effect handled elsewhere
            return model;

        case 'MARK_AUTO_STARTED':
            return { ...model, hasAutoStarted: true };

        // ====================================================================
        // PLATEAU
        // ====================================================================
        case 'UPDATE_PLATEAU_TILES':
            return {
                ...model,
                plateauTiles: msg.plateauTiles,
                availablePositions: msg.availablePositions,
            };

        case 'UPDATE_PLATEAU_OPTIMISTIC': {
            const playerId = model.session?.playerId;
            if (!playerId) return model;

            const currentTiles = model.plateauTiles[playerId] || Array(19).fill(null);
            const newTiles = [...currentTiles];

            // Format: "position:tile:image"
            newTiles[msg.position] = `${msg.position}:${msg.tile}:${msg.tileImage || ''}`;

            return {
                ...model,
                plateauTiles: {
                    ...model.plateauTiles,
                    [playerId]: newTiles,
                },
            };
        }

        // ====================================================================
        // CACHE
        // ====================================================================
        case 'UPDATE_IMAGE_CACHE':
            return {
                ...model,
                imageCache: msg.image,
                lastTileHash: msg.hash,
            };

        // ====================================================================
        // RESET
        // ====================================================================
        case 'RESET_GAME_STATE':
            return {
                ...model,
                gameState: null,
                currentTile: null,
                currentTileImage: null,
                plateauTiles: {},
                availablePositions: [],
                myTurn: false,
                isGameStarted: false,
                currentTurnNumber: 0,
                mctsLastMove: '',
                finalScores: null,
                imageCache: null,
                lastTileHash: '',
                hasAutoStarted: false,
            };

        case 'RESET_SESSION':
            return {
                ...initialModel,
                currentView: model.currentView,
                selectedGameMode: model.selectedGameMode,
            };

        default:
            // Exhaustive check
            const _exhaustive: never = msg;
            return model;
    }
};
