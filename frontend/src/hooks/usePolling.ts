// hooks/usePolling.ts - VERSION CORRIGÉE - Sans generateTileImagePath
import { gameClient } from '../services/GameClient';
import { onCleanup, batch } from 'solid-js';
import { SessionState } from '../generated/common';

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
    setFinalScores: (scores: Record<string, number> | null) => void,
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

    // ✅ INTERVALLES ULTRA-OPTIMISÉS POUR UX
    const getPollingInterval = (): number => {
        if (!isGameStarted()) return 20000;          // 20s en attente - réduire le bruit

        const timeSinceAction = Date.now() - lastActionTime;
        if (timeSinceAction < 1000) return 500;      // 500ms juste après action - très réactif
        if (timeSinceAction < 5000) return 2000;     // 2s dans les 5s après action 
        if (currentMyTurn) return 3000;              // 3s si mon tour
        return 12000;                                // 12s sinon - réduit drastiquement le polling
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
                            
                            // ✅ METTRE À JOUR LES SCORES EN TEMPS RÉEL
                            if (parsedState.scores) {
                                setGameState(prev => {
                                    if (!prev) return prev;
                                    
                                    const updatedPlayers = prev.players.map(p => ({
                                        ...p,
                                        score: parsedState.scores[p.id] || p.score
                                    }));
                                    
                                    // ✅ METTRE À JOUR LE MESSAGE DE STATUT AVEC LE NOUVEAU SCORE
                                    const currentSession = session();
                                    const mctsScore = parsedState.scores?.['mcts_ai'];
                                    if (currentSession) {
                                        const currentPlayer = updatedPlayers.find(p => p.id === currentSession.playerId);
                                        if (currentPlayer && currentPlayer.score > 0) {
                                            console.log('🏆 Score mis à jour frontend:', currentPlayer.score);
                                            const iaSegment =
                                                typeof mctsScore === 'number'
                                                    ? ` | 🤖 IA: ${mctsScore} pts`
                                                    : '';
                                            setStatusMessage(
                                                `🎯 Votre score actuel: ${currentPlayer.score} points${iaSegment}`
                                            );
                                        }
                                    }
                                    
                                    return {
                                        ...prev,
                                        players: updatedPlayers
                                    };
                                });
                            }
                        } catch (e) {
                            // Silencieux
                        }
                    }
                }

                // ✅ FIN DE PARTIE AVEC LOG UNIQUE
                if (result.isGameFinished && result.finalScores && result.finalScores !== "{}") {
                    try {
                        const scores = JSON.parse(result.finalScores);
                        
                        // ✅ AFFICHAGE PERSONNALISÉ POUR SINGLE-PLAYER
                        let scoreMessage = "🏁 Partie terminée ! ";
                        const playerIds = Object.keys(scores);
                        const mctsScore = scores["mcts_ai"];
                        const humanPlayer = playerIds.find(id => id !== "mcts_ai");
                        const humanScore = humanPlayer ? scores[humanPlayer] : 0;
                        
                        if (mctsScore !== undefined && humanPlayer) {
                            scoreMessage += `Vous: ${humanScore} pts | MCTS: ${mctsScore} pts`;
                            if (humanScore > mctsScore) {
                                scoreMessage += " 🎉 Victoire !";
                            } else if (humanScore < mctsScore) {
                                scoreMessage += " 🤖 MCTS gagne !";
                            } else {
                                scoreMessage += " 🤝 Égalité !";
                            }
                        } else {
                            scoreMessage += `Scores: ${JSON.stringify(scores)}`;
                        }
                        
                        setStatusMessage(scoreMessage);
                        setIsGameStarted(false);
                        setFinalScores(scores);
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
                if (convertedState.state !== SessionState.FINISHED) {
                    setFinalScores(null);
                }
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
