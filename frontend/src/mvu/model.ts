// mvu/model.ts - Le Model central (state immutable)
import { SessionState } from '../generated/common';

// ============================================================================
// AUTH TYPES
// ============================================================================

export interface User {
    id: string;
    email: string;
    username: string;
    emailVerified: boolean;
}

export interface AuthData {
    isAuthenticated: boolean;
    user: User | null;
    token: string | null;
    loginLoading: boolean;
    registerLoading: boolean;
    authError: string;
    authView: 'login' | 'register';
}

// ============================================================================
// TYPES DE DONNÉES
// ============================================================================

export interface Player {
    id: string;
    name: string;
    score: number;
    isReady: boolean;
    isConnected: boolean;
    joinedAt: string;
}

export interface GameStateData {
    sessionCode: string;
    state: SessionState;
    players: Player[];
    boardState: string;
    currentTurn?: string;
    gameMode?: string;
}

export interface Session {
    playerId: string;
    sessionCode: string;
    sessionId: string;
}

export interface GameMode {
    id: string;
    name: string;
    description: string;
    icon: string;
    simulations?: number;
    difficulty?: string;
}

// ============================================================================
// LOADING STATE - Granulaire par action
// ============================================================================

export interface LoadingState {
    createSession: boolean;
    joinSession: boolean;
    setReady: boolean;
    startTurn: boolean;
    playMove: boolean;
    polling: boolean;
}

// ============================================================================
// MODEL PRINCIPAL - State immutable de toute l'application
// ============================================================================

export interface Model {
    // Auth
    auth: AuthData;

    // Navigation
    currentView: 'login' | 'mode-selection' | 'game';
    selectedGameMode: GameMode | null;
    autoConnectSolo: boolean;

    // Session
    playerName: string;
    sessionCode: string;
    session: Session | null;
    gameState: GameStateData | null;

    // Gameplay
    currentTile: string | null;
    currentTileImage: string | null;
    plateauTiles: Record<string, string[]>;
    availablePositions: number[];
    myTurn: boolean;
    isGameStarted: boolean;
    currentTurnNumber: number;
    mctsLastMove: string;
    finalScores: Record<string, number> | null;

    // UI State
    loading: LoadingState;
    error: string;
    statusMessage: string;

    // MCTS Viewer
    isMctsViewer: boolean;
    hasAutoStarted: boolean;

    // Cache
    imageCache: string | null;
    lastTileHash: string;
}

// ============================================================================
// INITIAL STATE - État initial de l'application
// ============================================================================

export const initialModel: Model = {
    // Auth
    auth: {
        isAuthenticated: false,
        user: null,
        token: null,
        loginLoading: false,
        registerLoading: false,
        authError: '',
        authView: 'login',
    },

    // Navigation - commence par login
    currentView: 'login',
    selectedGameMode: null,
    autoConnectSolo: false,

    // Session
    playerName: '',
    sessionCode: '',
    session: null,
    gameState: null,

    // Gameplay
    currentTile: null,
    currentTileImage: null,
    plateauTiles: {},
    availablePositions: [],
    myTurn: false,
    isGameStarted: false,
    currentTurnNumber: 0,
    mctsLastMove: '',
    finalScores: null,

    // UI State
    loading: {
        createSession: false,
        joinSession: false,
        setReady: false,
        startTurn: false,
        playMove: false,
        polling: false,
    },
    error: '',
    statusMessage: '',

    // MCTS Viewer
    isMctsViewer: false,
    hasAutoStarted: false,

    // Cache
    imageCache: null,
    lastTileHash: '',
};

// ============================================================================
// FONCTIONS UTILITAIRES PURES
// ============================================================================

export const isLoading = (model: Model): boolean => {
    return Object.values(model.loading).some(v => v);
};

export const isPlayerReady = (model: Model): boolean => {
    if (!model.gameState || !model.session) return false;
    const player = model.gameState.players.find(p => p.id === model.session?.playerId);
    return player?.isReady || false;
};

export const isCurrentPlayer = (model: Model, playerId: string): boolean => {
    return model.session?.playerId === playerId;
};

export const getPlayerStatus = (player: Player): string => {
    return player.isReady ? "Prêt" : "En attente";
};

export const getSessionStateLabel = (state: SessionState): string => {
    switch (state) {
        case SessionState.WAITING: return "En attente";
        case SessionState.IN_PROGRESS: return "En cours";
        case SessionState.FINISHED: return "Terminée";
        case SessionState.CANCELLED: return "Annulée";
        default: return "Inconnue";
    }
};
