// mvu/store.ts - Store réactif qui connecte Model, Update et Effects
import { createSignal, createMemo, batch } from 'solid-js';
import { Model, initialModel, isLoading, isPlayerReady, getSessionStateLabel } from './model';
import type { Msg } from './messages';
import { msg } from './messages';
import { update } from './update';
import { getEffects, runEffect } from './effects';

// ============================================================================
// STORE TYPE
// ============================================================================

export interface Store {
    // Model getter (reactive)
    model: () => Model;

    // Dispatch function
    dispatch: (message: Msg) => void;

    // Auth accessors
    isAuthenticated: () => boolean;
    user: () => Model['auth']['user'];
    authToken: () => string | null;

    // Convenience accessors (reactive)
    currentView: () => Model['currentView'];
    selectedGameMode: () => Model['selectedGameMode'];
    playerName: () => string;
    sessionCode: () => string;
    session: () => Model['session'];
    gameState: () => Model['gameState'];
    currentTile: () => string | null;
    currentTileImage: () => string | null;
    plateauTiles: () => Model['plateauTiles'];
    availablePositions: () => number[];
    myTurn: () => boolean;
    isGameStarted: () => boolean;
    currentTurnNumber: () => number;
    error: () => string;
    statusMessage: () => string;
    isMctsViewer: () => boolean;
    autoConnectSolo: () => boolean;
    finalScores: () => Model['finalScores'];

    // Computed values
    isLoading: () => boolean;
    isPlayerReady: () => boolean;
    getSessionStateLabel: (state: number) => string;

    // Loading states
    loading: () => Model['loading'];

    // Message constructors (for convenience in views)
    msg: typeof msg;

    // Polling control
    startPolling: (sessionId: string) => void;
    stopPolling: () => void;
}

// ============================================================================
// CREATE STORE
// ============================================================================

export const createStore = (): Store => {
    // Signal principal contenant tout le state
    const [model, setModel] = createSignal<Model>(initialModel);

    // Polling interval
    let pollingInterval: number | null = null;

    // Dispatch function - le coeur du MVU
    const dispatch = (message: Msg): void => {
        const currentModel = model();

        // 1. Déterminer les effets AVANT l'update (basé sur l'ancien état)
        const effect = getEffects(currentModel, message);

        // 2. Update the model
        batch(() => {
            const newModel = update(currentModel, message);
            setModel(newModel);
        });

        // 3. Run effects APRÈS l'update (hors du batch pour éviter les problèmes)
        if (effect.type !== 'NONE') {
            runEffect(effect, dispatch);
        }
    };

    // Polling functions
    const startPolling = (sessionId: string): void => {
        if (pollingInterval) {
            clearInterval(pollingInterval);
        }

        // Poll every 2 seconds
        pollingInterval = window.setInterval(() => {
            runEffect(
                { type: 'POLL_STATE', sessionId },
                dispatch
            );
        }, 2000);
    };

    const stopPolling = (): void => {
        if (pollingInterval) {
            clearInterval(pollingInterval);
            pollingInterval = null;
        }
    };

    // Convenience accessors
    const store: Store = {
        model,
        dispatch,

        // Auth accessors
        isAuthenticated: createMemo(() => model().auth.isAuthenticated),
        user: createMemo(() => model().auth.user),
        authToken: createMemo(() => model().auth.token),

        // Accessors
        currentView: createMemo(() => model().currentView),
        selectedGameMode: createMemo(() => model().selectedGameMode),
        playerName: createMemo(() => model().playerName),
        sessionCode: createMemo(() => model().sessionCode),
        session: createMemo(() => model().session),
        gameState: createMemo(() => model().gameState),
        currentTile: createMemo(() => model().currentTile),
        currentTileImage: createMemo(() => model().currentTileImage),
        plateauTiles: createMemo(() => model().plateauTiles),
        availablePositions: createMemo(() => model().availablePositions),
        myTurn: createMemo(() => model().myTurn),
        isGameStarted: createMemo(() => model().isGameStarted),
        currentTurnNumber: createMemo(() => model().currentTurnNumber),
        error: createMemo(() => model().error),
        statusMessage: createMemo(() => model().statusMessage),
        isMctsViewer: createMemo(() => model().isMctsViewer),
        autoConnectSolo: createMemo(() => model().autoConnectSolo),
        finalScores: createMemo(() => model().finalScores),
        loading: createMemo(() => model().loading),

        // Computed
        isLoading: createMemo(() => isLoading(model())),
        isPlayerReady: createMemo(() => isPlayerReady(model())),
        getSessionStateLabel,

        // Message constructors
        msg,

        // Polling
        startPolling,
        stopPolling,
    };

    return store;
};

// ============================================================================
// SINGLETON STORE INSTANCE
// ============================================================================

let storeInstance: Store | null = null;

export const getStore = (): Store => {
    if (!storeInstance) {
        storeInstance = createStore();
    }
    return storeInstance;
};

// Reset store (for testing)
export const resetStore = (): void => {
    if (storeInstance) {
        storeInstance.stopPolling();
    }
    storeInstance = null;
};
