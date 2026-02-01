// mvu/messages.ts - Tous les messages/actions possibles
import type { GameStateData, Session, Player, GameMode, User } from './model';

// ============================================================================
// MESSAGE TYPES - Union discriminée de tous les événements
// ============================================================================

// Auth
export type LoginRequest = { type: 'LOGIN_REQUEST'; email: string; password: string };
export type LoginSuccess = { type: 'LOGIN_SUCCESS'; user: User; token: string };
export type LoginFailure = { type: 'LOGIN_FAILURE'; error: string };
export type RegisterRequest = { type: 'REGISTER_REQUEST'; email: string; username: string; password: string };
export type RegisterSuccess = { type: 'REGISTER_SUCCESS'; user: User; token: string };
export type RegisterFailure = { type: 'REGISTER_FAILURE'; error: string };
export type LogoutRequest = { type: 'LOGOUT_REQUEST' };
export type LogoutComplete = { type: 'LOGOUT_COMPLETE' };
export type CheckAuthRequest = { type: 'CHECK_AUTH_REQUEST' };
export type CheckAuthSuccess = { type: 'CHECK_AUTH_SUCCESS'; user: User; token: string };
export type CheckAuthFailure = { type: 'CHECK_AUTH_FAILURE' };
export type SwitchAuthView = { type: 'SWITCH_AUTH_VIEW'; view: 'login' | 'register' };
export type ClearAuthError = { type: 'CLEAR_AUTH_ERROR' };
export type SkipAuth = { type: 'SKIP_AUTH' };

// Navigation
export type NavigateToGame = { type: 'NAVIGATE_TO_GAME'; gameMode: GameMode; autoConnect: boolean };
export type NavigateToModeSelection = { type: 'NAVIGATE_TO_MODE_SELECTION' };

// Input utilisateur
export type SetPlayerName = { type: 'SET_PLAYER_NAME'; name: string };
export type SetSessionCode = { type: 'SET_SESSION_CODE'; code: string };

// Session - Requêtes
export type CreateSessionRequest = { type: 'CREATE_SESSION_REQUEST' };
export type JoinSessionRequest = { type: 'JOIN_SESSION_REQUEST' };
export type SetReadyRequest = { type: 'SET_READY_REQUEST' };
export type LeaveSessionRequest = { type: 'LEAVE_SESSION_REQUEST' };

// Session - Réponses
export type CreateSessionSuccess = {
    type: 'CREATE_SESSION_SUCCESS';
    session: Session;
    gameState: GameStateData;
};
export type CreateSessionFailure = { type: 'CREATE_SESSION_FAILURE'; error: string };
export type JoinSessionSuccess = {
    type: 'JOIN_SESSION_SUCCESS';
    session: Session;
    gameState: GameStateData;
};
export type JoinSessionFailure = { type: 'JOIN_SESSION_FAILURE'; error: string };
export type SetReadySuccess = { type: 'SET_READY_SUCCESS'; gameStarted: boolean };
export type SetReadyFailure = { type: 'SET_READY_FAILURE'; error: string };
export type LeaveSessionComplete = { type: 'LEAVE_SESSION_COMPLETE' };

// Gameplay - Requêtes
export type StartTurnRequest = { type: 'START_TURN_REQUEST' };
export type PlayMoveRequest = { type: 'PLAY_MOVE_REQUEST'; position: number };

// Gameplay - Réponses
export type StartTurnSuccess = {
    type: 'START_TURN_SUCCESS';
    tile: string;
    tileImage: string;
    turnNumber: number;
    waitingForPlayers: string[];
    gameState?: string;
};
export type StartTurnFailure = { type: 'START_TURN_FAILURE'; error: string };
export type PlayMoveSuccess = {
    type: 'PLAY_MOVE_SUCCESS';
    position: number;
    pointsEarned: number;
    newGameState: any;
    mctsResponse?: string;
    isGameOver: boolean;
};
export type PlayMoveFailure = { type: 'PLAY_MOVE_FAILURE'; error: string };

// Polling - État du jeu
export type PollStateSuccess = {
    type: 'POLL_STATE_SUCCESS';
    gameState: GameStateData;
    currentTile?: string;
    currentTileImage?: string;
    turnNumber?: number;
    isGameOver?: boolean;
    finalScores?: Record<string, number>;
};
export type PollStateFailure = { type: 'POLL_STATE_FAILURE'; error: string };

// UI
export type ClearError = { type: 'CLEAR_ERROR' };
export type ClearStatusMessage = { type: 'CLEAR_STATUS_MESSAGE' };
export type SetStatusMessage = { type: 'SET_STATUS_MESSAGE'; message: string };
export type SetError = { type: 'SET_ERROR'; error: string };

// MCTS Viewer
export type SetMctsViewerMode = { type: 'SET_MCTS_VIEWER_MODE'; enabled: boolean };
export type OpenMctsSession = { type: 'OPEN_MCTS_SESSION' };

// Auto-start
export type MarkAutoStarted = { type: 'MARK_AUTO_STARTED' };

// Plateau updates
export type UpdatePlateauTiles = {
    type: 'UPDATE_PLATEAU_TILES';
    plateauTiles: Record<string, string[]>;
    availablePositions: number[];
};
export type UpdatePlateauOptimistic = {
    type: 'UPDATE_PLATEAU_OPTIMISTIC';
    position: number;
    tile: string;
    tileImage?: string;
};

// Image cache
export type UpdateImageCache = { type: 'UPDATE_IMAGE_CACHE'; image: string; hash: string };

// Reset
export type ResetGameState = { type: 'RESET_GAME_STATE' };
export type ResetSession = { type: 'RESET_SESSION' };

// ============================================================================
// MSG - Union de tous les messages
// ============================================================================

export type Msg =
    // Auth
    | LoginRequest
    | LoginSuccess
    | LoginFailure
    | RegisterRequest
    | RegisterSuccess
    | RegisterFailure
    | LogoutRequest
    | LogoutComplete
    | CheckAuthRequest
    | CheckAuthSuccess
    | CheckAuthFailure
    | SwitchAuthView
    | ClearAuthError
    | SkipAuth
    // Navigation
    | NavigateToGame
    | NavigateToModeSelection
    // Input
    | SetPlayerName
    | SetSessionCode
    // Session
    | CreateSessionRequest
    | CreateSessionSuccess
    | CreateSessionFailure
    | JoinSessionRequest
    | JoinSessionSuccess
    | JoinSessionFailure
    | SetReadyRequest
    | SetReadySuccess
    | SetReadyFailure
    | LeaveSessionRequest
    | LeaveSessionComplete
    // Gameplay
    | StartTurnRequest
    | StartTurnSuccess
    | StartTurnFailure
    | PlayMoveRequest
    | PlayMoveSuccess
    | PlayMoveFailure
    // Polling
    | PollStateSuccess
    | PollStateFailure
    // UI
    | ClearError
    | ClearStatusMessage
    | SetStatusMessage
    | SetError
    // MCTS
    | SetMctsViewerMode
    | OpenMctsSession
    | MarkAutoStarted
    // Plateau
    | UpdatePlateauTiles
    | UpdatePlateauOptimistic
    // Cache
    | UpdateImageCache
    // Reset
    | ResetGameState
    | ResetSession;

// ============================================================================
// MESSAGE CONSTRUCTORS - Fonctions pour créer des messages
// ============================================================================

export const msg = {
    // Auth
    login: (email: string, password: string): Msg => ({ type: 'LOGIN_REQUEST', email, password }),
    register: (email: string, username: string, password: string): Msg => ({
        type: 'REGISTER_REQUEST',
        email,
        username,
        password,
    }),
    logout: (): Msg => ({ type: 'LOGOUT_REQUEST' }),
    checkAuth: (): Msg => ({ type: 'CHECK_AUTH_REQUEST' }),
    switchAuthView: (view: 'login' | 'register'): Msg => ({ type: 'SWITCH_AUTH_VIEW', view }),
    clearAuthError: (): Msg => ({ type: 'CLEAR_AUTH_ERROR' }),
    skipAuth: (): Msg => ({ type: 'SKIP_AUTH' }),

    // Navigation
    navigateToGame: (gameMode: GameMode, autoConnect: boolean): Msg => ({
        type: 'NAVIGATE_TO_GAME',
        gameMode,
        autoConnect,
    }),
    navigateToModeSelection: (): Msg => ({ type: 'NAVIGATE_TO_MODE_SELECTION' }),

    // Input
    setPlayerName: (name: string): Msg => ({ type: 'SET_PLAYER_NAME', name }),
    setSessionCode: (code: string): Msg => ({ type: 'SET_SESSION_CODE', code }),

    // Session
    createSession: (): Msg => ({ type: 'CREATE_SESSION_REQUEST' }),
    joinSession: (): Msg => ({ type: 'JOIN_SESSION_REQUEST' }),
    setReady: (): Msg => ({ type: 'SET_READY_REQUEST' }),
    leaveSession: (): Msg => ({ type: 'LEAVE_SESSION_REQUEST' }),

    // Gameplay
    startTurn: (): Msg => ({ type: 'START_TURN_REQUEST' }),
    playMove: (position: number): Msg => ({ type: 'PLAY_MOVE_REQUEST', position }),

    // UI
    clearError: (): Msg => ({ type: 'CLEAR_ERROR' }),
    clearStatus: (): Msg => ({ type: 'CLEAR_STATUS_MESSAGE' }),
    setStatus: (message: string): Msg => ({ type: 'SET_STATUS_MESSAGE', message }),
    setError: (error: string): Msg => ({ type: 'SET_ERROR', error }),

    // MCTS
    setMctsViewer: (enabled: boolean): Msg => ({ type: 'SET_MCTS_VIEWER_MODE', enabled }),
    openMctsSession: (): Msg => ({ type: 'OPEN_MCTS_SESSION' }),
    markAutoStarted: (): Msg => ({ type: 'MARK_AUTO_STARTED' }),

    // Reset
    resetGameState: (): Msg => ({ type: 'RESET_GAME_STATE' }),
    resetSession: (): Msg => ({ type: 'RESET_SESSION' }),
};
