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
    };

    // ✅ INTERVALLES FORTEMENT AUGMENTÉS - Moins de spam réseau
    const getPollingInterval = (): number => {
        if (!isGameStarted()) return 15000;          // 15s en attente (doublé)

        const timeSinceAction = Date.now() - lastActionTime;
        if (timeSinceAction < 3000) return 3000;     // 3s après action (plus long)
        if (currentMyTurn) return 5000;              // 5s mon tour  
        return 8000;                                 // 8s normal (plus long)
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

                // ✅ GESTION TUILE AVEC LOGS CONDITIONNELS
                const newTile = result.currentTile;
                const newTileImage = result.currentTileImage;
                const currentTileValue = currentTile();

                if (newTile && newTile !== currentTileValue) {
                    // ✅ LOG DÉSACTIVÉ - Évite spam console
                    // console.log('🎲 Nouvelle tuile détectée:', newTile);
                    setCurrentTile(newTile);
                    setCurrentTileImage(newTileImage || null);
                    markActionPerformed();

                } else if (!newTile && currentTileValue) {
                    const timeSinceAction = Date.now() - lastActionTime;
                    if (timeSinceAction > 10000) { // Plus long pour éviter les resets prématurés
                        setCurrentTile(null);
                        setCurrentTileImage(null);
                    }
                }

                // ✅ GESTION DU TOUR AVEC LOGS RÉDUITS
                const currentSession = session();
                if (currentSession) {
                    const newMyTurn = result.waitingForPlayers?.includes(currentSession.playerId) || false;

                    if (newMyTurn !== currentMyTurn) {
                        currentMyTurn = newMyTurn;
                        setMyTurn(newMyTurn);

                        // ✅ LOG DÉSACTIVÉ - Évite spam console
                        // console.log('🎯 À votre tour !', newMyTurn);

                        if (newMyTurn) {
                            markActionPerformed();
                        }
                    }
                }

                // ✅ PLATEAU - MISE À JOUR SANS LOGS RÉPÉTITIFS
                if (result.gameState) {
                    const timeSinceAction = Date.now() - lastActionTime;

                    if (timeSinceAction > 200) { // Légèrement plus long
                        try {
                            const parsedState = JSON.parse(result.gameState);
                            updatePlateauTiles(parsedState);
                        } catch (e) {
                            // Silencieux
                        }
                    }
                }

                // ✅ FIN DE PARTIE AVEC LOG UNIQUE
                if (result.isGameFinished && result.finalScores && result.finalScores !== "{}") {
                    try {
                        const scores = JSON.parse(result.finalScores);
                        setStatusMessage(`🏁 Partie terminée ! Scores: ${JSON.stringify(scores, null, 2)}`);
                        setIsGameStarted(false);
                        console.log('🏁 Partie terminée avec scores:', scores);
                    } catch (e) {
                        setStatusMessage(`🏁 Jeu terminé !`);
                        setIsGameStarted(false);
                    }
                }

            } else {
                consecutiveErrors++;
                // ✅ LOG D'ERREUR SEULEMENT APRÈS PLUSIEURS ÉCHECS
                if (consecutiveErrors > 3 && process.env.NODE_ENV === 'development') {
                    console.warn('⚠️ Erreurs de polling consécutives:', consecutiveErrors);
                }
            }
        } catch (error) {
            consecutiveErrors++;
            // ✅ LOG D'ERREUR SEULEMENT SI CRITIQUE
            if (consecutiveErrors > 5 && process.env.NODE_ENV === 'development') {
                console.error('❌ Erreur critique de polling:', error);
            }
        }
    };

    // ============================================================================
    // POLLING SESSION - VERSION SIMPLIFIÉE
    // ============================================================================

    const pollSessionState = async (sessionId: string) => {
        try {
            const sessionResult = await gameClient.getSessionState(sessionId);

            if (sessionResult.success && sessionResult.sessionState) {
                const convertedState = convertSessionState(sessionResult.sessionState);
                setGameState(convertedState);
                // ✅ AUCUN LOG - Session polling silencieux
            }
        } catch (error) {
            // ✅ SILENCIEUX SAUF EN DEBUG
            if (process.env.NODE_ENV === 'development' && consecutiveErrors > 5) {
                console.warn('Session polling error:', error);
            }
        }
    };

    // ============================================================================
    // DÉMARRAGE POLLING - VERSION SIMPLIFIÉE
    // ============================================================================

    const startPolling = (sessionId: string) => {
        if (!sessionId || typeof sessionId !== 'string' || sessionId.trim() === '') {
            return;
        }

        stopPolling();

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
                        }
                    } catch (e) {
                        // Silencieux pour éviter spam
                    }
                }

            } catch (error) {
                consecutiveErrors++;
            }

            // ✅ PROGRAMMATION SIMPLE du prochain poll
            const baseInterval = getPollingInterval();
            const finalInterval = getErrorAdjustedInterval(baseInterval);

            pollInterval = window.setTimeout(poll, finalInterval);

            // Debug seulement si erreurs
            if (consecutiveErrors > 0) {
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
        }
    };

    const resetPollingState = () => {
        consecutiveErrors = 0;
        lastActionTime = 0;
        currentMyTurn = false;
    };

    const forceRefresh = async () => {
        const currentSession = session();
        if (currentSession) {
            markActionPerformed();
            await pollGameplayState(currentSession.sessionId);
        }
    };

    // ============================================================================
    // NETTOYAGE
    // ============================================================================

    onCleanup(() => {
        stopPolling();
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