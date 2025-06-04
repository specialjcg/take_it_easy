// hooks/useGameState.ts - State management centralisé
import { createSignal } from 'solid-js';
import { SessionState } from '../generated/common';

// Types pour l'état local (réutilisés du composant principal)
export interface Player {
    id: string;
    name: string;
    score: number;
    isReady: boolean;
    isConnected: boolean;
    joinedAt: string;
}

export interface GameState {
    sessionCode: string;
    state: SessionState;
    players: Player[];
    boardState: string;
    currentTurn?: string;
}

export interface Session {
    playerId: string;
    sessionCode: string;
    sessionId: string;
}

/**
 * Hook centralisé pour la gestion d'état du jeu
 * Remplace les multiples createSignal() dispersés dans le composant principal
 */
export const useGameState = () => {
    // État de session
    const [playerName, setPlayerName] = createSignal('');
    const [sessionCode, setSessionCode] = createSignal('');
    const [gameState, setGameState] = createSignal<GameState | null>(null);
    const [session, setSession] = createSignal<Session | null>(null);
    
    // État UI
    const [loading, setLoading] = createSignal(false);
    const [error, setError] = createSignal('');
    const [statusMessage, setStatusMessage] = createSignal('');
    
    // État du gameplay
    const [currentTile, setCurrentTile] = createSignal<string | null>(null);
    const [currentTileImage, setCurrentTileImage] = createSignal<string | null>(null);
    const [plateauTiles, setPlateauTiles] = createSignal<{[playerId: string]: string[]}>({});
    const [availablePositions, setAvailablePositions] = createSignal<number[]>([]);
    const [myTurn, setMyTurn] = createSignal(false);
    const [isGameStarted, setIsGameStarted] = createSignal(false);
    const [currentTurnNumber, setCurrentTurnNumber] = createSignal(0);
    const [mctsLastMove, setMctsLastMove] = createSignal<string>('');
    
    // État debug
    const [showDebugLogs, setShowDebugLogs] = createSignal(false);
    const [debugLogs, setDebugLogs] = createSignal<string[]>([]);
    
    // Cache pour les images
    const [imageCache, setImageCache] = createSignal<string | null>(null);
    const [lastTileHash, setLastTileHash] = createSignal<string>('');

    // Fonctions utilitaires pour l'état
    const addDebugLog = (message: string) => {
        if (showDebugLogs()) {
            const timestamp = new Date().toLocaleTimeString();
            const logEntry = `${timestamp}: ${message}`;
            setDebugLogs(prev => [logEntry, ...prev.slice(0, 15)]);
        }
    };

    const clearError = () => setError('');
    const clearStatusMessage = () => setStatusMessage('');
    
    const resetGameState = () => {
        setGameState(null);
        setCurrentTile(null);
        setCurrentTileImage(null);
        setPlateauTiles({});
        setAvailablePositions([]);
        setMyTurn(false);
        setIsGameStarted(false);
        setCurrentTurnNumber(0);
        setMctsLastMove('');
        setImageCache(null);
        setLastTileHash('');
    };

    const resetSession = () => {
        setSession(null);
        setPlayerName('');
        setSessionCode('');
        resetGameState();
        clearError();
        clearStatusMessage();
    };

    // Getters dérivés pour éviter la duplication de logique
    const isPlayerReady = () => {
        const state = gameState();
        const currentSession = session();
        if (!state || !currentSession) return false;
        
        const player = state.players.find(p => p.id === currentSession.playerId);
        return player?.isReady || false;
    };

    const isCurrentPlayer = (playerId: string) => {
        const currentSession = session();
        return currentSession?.playerId === playerId;
    };

    const getPlayerStatus = (player: Player) => {
        if (player.isReady) {
            return "✅ Prêt";
        }
        return "⏳ En attente";
    };

    const getSessionStateLabel = (state: SessionState) => {
        switch (state) {
            case SessionState.WAITING: return "En attente";
            case SessionState.IN_PROGRESS: return "En cours";
            case SessionState.FINISHED: return "Terminée";
            case SessionState.CANCELLED: return "Annulée";
            default: return "Inconnue";
        }
    };

    return {
        // État de session
        playerName, setPlayerName,
        sessionCode, setSessionCode,
        gameState, setGameState,
        session, setSession,
        
        // État UI
        loading, setLoading,
        error, setError,
        statusMessage, setStatusMessage,
        
        // État du gameplay
        currentTile, setCurrentTile,
        currentTileImage, setCurrentTileImage,
        plateauTiles, setPlateauTiles,
        availablePositions, setAvailablePositions,
        myTurn, setMyTurn,
        isGameStarted, setIsGameStarted,
        currentTurnNumber, setCurrentTurnNumber,
        mctsLastMove, setMctsLastMove,
        
        // État debug
        showDebugLogs, setShowDebugLogs,
        debugLogs, setDebugLogs,
        addDebugLog,
        
        // Cache images
        imageCache, setImageCache,
        lastTileHash, setLastTileHash,
        
        // Fonctions utilitaires
        clearError,
        clearStatusMessage,
        resetGameState,
        resetSession,
        
        // Getters dérivés
        isPlayerReady,
        isCurrentPlayer,
        getPlayerStatus,
        getSessionStateLabel
    };
};