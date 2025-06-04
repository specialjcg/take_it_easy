// hooks/usePolling.ts - VERSION CORRIGÉE - Sans generateTileImagePath
import { gameClient } from '../services/GameClient';
import { onCleanup, batch } from 'solid-js';

/**
 * Hook pour la gestion du polling de l'état du jeu
 * VERSION SIMPLIFIÉE qui fonctionne de manière fiable
 */
export const usePolling = (
    session: () => { sessionId: string; playerId: string } | null,
    isGameStarted: () => boolean,
    currentTile: () => string | null,
    setGameState: (updater: any) => void,
    setCurrentTile: (tile: string | null) => void,
    setCurrentTileImage: (image: string | null) => void,
    setMyTurn: (turn: boolean) => void,
    setIsGameStarted: (started: boolean) => void,
    setStatusMessage: (message: string) => void,
    updatePlateauTiles: (gameState: any) => void,
    convertSessionState: (sessionState: any) => any,
    addDebugLog: (message: string) => void
) => {
    // ============================================================================
    // VARIABLES D'ÉTAT SIMPLIFIÉES
    // ============================================================================
    let pollInterval: number | undefined;
    let consecutiveErrors = 0;
    let lastActionTime = 0;
    let currentMyTurn = false;

    // ============================================================================
    // FONCTION UTILITAIRE LOCALE (temporaire)
    // ============================================================================

    // ✅ Fonction locale uniquement pour la tuile courante (en attendant backend complet)


    // ============================================================================
    // FONCTIONS UTILITAIRES SIMPLIFIÉES
    // ============================================================================

    const markActionPerformed = () => {
        lastActionTime = Date.now();
        addDebugLog(`⚡ Action marquée: ${lastActionTime}`);
    };

    // ✅ INTERVALLES MODÉRÉS (pas trop agressifs)
    const getPollingInterval = (): number => {
        if (!isGameStarted()) return 4000;           // 4s en attente

        const timeSinceAction = Date.now() - lastActionTime;
        if (timeSinceAction < 5000) return 800;      // 800ms après action
        if (currentMyTurn) return 1500;              // 1.5s mon tour
        return 3000;                                 // 3s normal
    };

    // ✅ BACKOFF MODÉRÉ
    const getErrorAdjustedInterval = (baseInterval: number): number => {
        if (consecutiveErrors === 0) return baseInterval;
        return baseInterval * Math.min(Math.pow(1.5, consecutiveErrors), 8);
    };

    // ============================================================================
    // POLLING GAMEPLAY - VERSION SIMPLIFIÉE ET FIABLE
    // ============================================================================

    const pollGameplayState = async (sessionId: string) => {
        if (!sessionId || typeof sessionId !== 'string' || sessionId.trim() === '') {
            return;
        }

        try {
            const result = await gameClient.getGameState(sessionId);

            if (result.success) {
                consecutiveErrors = 0;

                // ✅ GESTION TUILE SIMPLIFIÉE - Utilise fonction locale
                const newTile = result.currentTile;
                const newTileImage = result.currentTileImage; // ✅ BACKEND DIRECT!
                const currentTileValue = currentTile();

                if (newTile && newTile !== currentTileValue) {
                    // Nouvelle tuile détectée
                    setCurrentTile(newTile);
                    setCurrentTileImage(newTileImage || null); // ✅ BACKEND IMAGE!
                    markActionPerformed();
                    addDebugLog(`📡 Nouvelle tuile: ${newTile} → ${newTileImage}`);

                } else if (!newTile && currentTileValue) {
                    // Pas de tuile courante
                    const timeSinceAction = Date.now() - lastActionTime;
                    if (timeSinceAction > 8000) {
                        setCurrentTile(null);
                        setCurrentTileImage(null);
                        addDebugLog(`⚠️ Clear tuile après 8s`);
                    }
                }

                // ✅ GESTION DU TOUR SIMPLIFIÉE
                const currentSession = session();
                if (currentSession) {
                    const newMyTurn = result.waitingForPlayers?.includes(currentSession.playerId) || false;

                    if (newMyTurn !== currentMyTurn) {
                        currentMyTurn = newMyTurn;
                        setMyTurn(newMyTurn);
                        addDebugLog(`🔄 Tour: ${newMyTurn ? 'MON TOUR' : 'Attente'}`);

                        if (newMyTurn) {
                            markActionPerformed();
                        }
                    }
                }

                // ✅ PLATEAU - MISE À JOUR DIRECTE (utilise backend via updatePlateauTiles)
                if (result.gameState) {
                    const timeSinceAction = Date.now() - lastActionTime;

                    // ✅ CONDITION SIMPLIFIÉE - toujours mettre à jour après 2s
                    if (timeSinceAction > 2000) {
                        try {
                            const parsedState = JSON.parse(result.gameState);
                            updatePlateauTiles(parsedState); // ✅ Cette fonction utilise les données backend
                            addDebugLog(`🔄 Plateau mis à jour (${timeSinceAction}ms depuis action)`);
                        } catch (e) {
                            addDebugLog(`❌ Erreur parse plateau: ${e}`);
                        }
                    } else {
                        addDebugLog(`⏳ Protection optimiste: ${timeSinceAction}ms`);
                    }
                }

                // ✅ FIN DE PARTIE
                if (result.isGameFinished && result.finalScores && result.finalScores !== "{}") {
                    try {
                        const scores = JSON.parse(result.finalScores);
                        setStatusMessage(`🏁 Terminé ! Scores: ${JSON.stringify(scores, null, 2)}`);
                        setIsGameStarted(false);
                        addDebugLog('🏁 Jeu terminé');
                    } catch (e) {
                        setStatusMessage(`🏁 Jeu terminé !`);
                        setIsGameStarted(false);
                    }
                }

                // ✅ DEBUG SIMPLE
                addDebugLog(`📊 Poll OK - Tuile: ${newTile || 'none'}, Tour: ${currentMyTurn}, Attente: ${result.waitingForPlayers?.length || 0}`);

            } else {
                consecutiveErrors++;
                addDebugLog(`❌ Poll error #${consecutiveErrors}: ${result.error || 'Unknown'}`);
            }
        } catch (error) {
            consecutiveErrors++;
            addDebugLog(`💥 Poll exception #${consecutiveErrors}: ${error}`);
        }
    };

    // ============================================================================
    // POLLING SESSION - VERSION SIMPLIFIÉE
    // ============================================================================

    const pollSessionState = async (sessionId: string) => {
        try {
            const sessionResult = await gameClient.getSessionState(sessionId);

            if (sessionResult.success && sessionResult.sessionState) {
                // ✅ MISE À JOUR DIRECTE
                const convertedState = convertSessionState(sessionResult.sessionState);
                setGameState(convertedState);
                // Pas de log pour éviter le spam
            }
        } catch (error) {
            addDebugLog(`❌ Session poll error: ${error}`);
        }
    };

    // ============================================================================
    // DÉMARRAGE POLLING - VERSION SIMPLIFIÉE
    // ============================================================================

    const startPolling = (sessionId: string) => {
        if (!sessionId || typeof sessionId !== 'string' || sessionId.trim() === '') {
            addDebugLog(`❌ sessionId invalide: "${sessionId}"`);
            return;
        }

        stopPolling();
        addDebugLog(`🔄 Démarrage polling pour: ${sessionId.slice(0, 8)}...`);

        const poll = async () => {
            try {
                // ✅ SESSION EN PREMIER (léger)
                await pollSessionState(sessionId);

                // ✅ GAMEPLAY selon l'état
                if (isGameStarted()) {
                    await pollGameplayState(sessionId);
                } else {
                    // ✅ DÉTECTION NOUVELLE TUILE
                    try {
                        const gameplayResult = await gameClient.getGameplayState(sessionId);
                        if (gameplayResult.success && gameplayResult.currentTile && !currentTile()) {
                            // ✅ MISE À JOUR DIRECTE - Utilise fonction locale
                            setCurrentTile(gameplayResult.currentTile);
                            setCurrentTileImage(gameplayResult.currentTileImage || null);
                            setIsGameStarted(true);
                            markActionPerformed();
                            addDebugLog(`📡 Tuile détectée: ${gameplayResult.currentTile}`);
                        }
                    } catch (e) {
                        // Silencieux pour éviter spam
                    }
                }

            } catch (error) {
                consecutiveErrors++;
                addDebugLog(`💥 Poll général #${consecutiveErrors}: ${error}`);
            }

            // ✅ PROGRAMMATION SIMPLE du prochain poll
            const baseInterval = getPollingInterval();
            const finalInterval = getErrorAdjustedInterval(baseInterval);

            pollInterval = window.setTimeout(poll, finalInterval);

            // Debug seulement si erreurs
            if (consecutiveErrors > 0) {
                addDebugLog(`⏰ Prochain poll: ${finalInterval}ms (erreurs: ${consecutiveErrors})`);
            }
        };

        // Démarrage immédiat
        poll();
    };

    // ============================================================================
    // ARRÊT ET UTILITAIRES
    // ============================================================================

    const stopPolling = () => {
        if (pollInterval) {
            clearTimeout(pollInterval);
            pollInterval = undefined;
            addDebugLog('🛑 Polling arrêté');
        }
    };

    const resetPollingState = () => {
        consecutiveErrors = 0;
        lastActionTime = 0;
        currentMyTurn = false;
        addDebugLog('🔄 État polling reset');
    };

    const forceRefresh = async () => {
        const currentSession = session();
        if (currentSession) {
            markActionPerformed();
            await pollGameplayState(currentSession.sessionId);
            addDebugLog('🔄 Refresh forcé');
        }
    };

    // ============================================================================
    // NETTOYAGE
    // ============================================================================

    onCleanup(() => {
        stopPolling();
        addDebugLog('🧹 usePolling cleanup');
    });

    // ============================================================================
    // API PUBLIQUE SIMPLIFIÉE
    // ============================================================================

    return {
        startPolling,
        stopPolling,
        markActionPerformed,
        pollGameplayState,
        resetPollingState,
        forceRefresh,
        isPolling: () => pollInterval !== undefined,
        getStats: () => ({
            consecutiveErrors,
            lastActionTime,
            currentMyTurn
        })
    };
};

// ============================================================================
// NOTE: FONCTION LOCALE TEMPORAIRE
// ============================================================================

/*
🔧 FONCTION getTileImagePath() LOCALE

Cette fonction est temporaire et seulement utilisée pour la "tuile courante"
(la tuile annoncée qu'on voit en haut de l'écran).

Les plateaux utilisent maintenant 100% les données du backend via
GameStateManager.updatePlateauTiles() qui utilise plateau.tile_images.

À terme, le backend devrait aussi retourner l'image de la tuile courante
directement dans currentTileImage au lieu de currentTile.
*/