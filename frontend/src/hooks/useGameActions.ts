// hooks/useGameActions.ts - Actions gameplay isolées
import { gameClient } from '../services/GameClient';
import type { GameState, Session } from './useGameState';
import {batch} from "solid-js";

/**
 * Hook pour les actions de gameplay (gRPC calls)
 * Isole toute la logique métier du composant principal
 */
export const useGameActions = (
    session: () => Session | null,
    loadingManager: ReturnType<typeof import('./useLoadingManager').useLoadingManager>,
    setError: (error: string) => void,
    setStatusMessage: (message: string) => void,
    setCurrentTile: (tile: string | null) => void,
    setCurrentTileImage: (image: string | null) => void,
    setCurrentTurnNumber: (turn: number) => void,
    setIsGameStarted: (started: boolean) => void,
    setMyTurn: (turn: boolean) => void,
    setMctsLastMove: (move: string) => void,
    updatePlateauTiles: (gameState: any) => void,
) => {

    /**
     * Démarrer un nouveau tour (tire une tuile aléatoire)
     */
        // ✅ SOLUTION 2: startGameTurn optimisé
    const startGameTurn = async () => {
            const currentSession = session();
            if (!currentSession) return;

            // ✅ BATCH: État de chargement centralisé
            batch(() => {
                loadingManager.setLoading('start-turn', true);
                setError('');
            });

            try {
                const result = await gameClient.startNewTurn(currentSession.sessionId);

                if (result.success) {
                    // ✅ BATCH: Mise à jour complète du tour
                    batch(() => {
                        setCurrentTile(result.announcedTile || null);
                        setCurrentTileImage(result.tileImage || null);
                        setCurrentTurnNumber(result.turnNumber || 0);
                        setStatusMessage(`🎲 Tour ${result.turnNumber}: ${result.announcedTile}`);
                        setIsGameStarted(true);
                        setMyTurn(result.waitingForPlayers?.includes(currentSession.playerId) || false);
                        loadingManager.setLoading('start-turn', false);
                    });

                    // ✅ PLATEAU EN DIFFÉRÉ (non-bloquant)
                    if (result.gameState) {
                        queueMicrotask(() => {
                            const parsedState = JSON.parse(result.gameState);
                            updatePlateauTiles(parsedState);
                        });
                    }
                } else {
                    batch(() => {
                        setError(result.error || 'Erreur tour');
                        loadingManager.setLoading('start-turn', false);
                    });
                }
            } catch (error) {
                batch(() => {
                    setError('Erreur connexion');
                    loadingManager.setLoading('start-turn', false);
                });
            }
        };

    /**
     * Jouer un mouvement (position sur le plateau) - VERSION OPTIMISTE
     */
    const playMove = async (position: number, myTurn: () => boolean, markActionPerformed?: () => void) => {
        const currentSession = session();

        if (!currentSession || !myTurn()) {
            setStatusMessage("Ce n'est pas votre tour !");
            return;
        }

        batch(() => {
            setStatusMessage(`🎯 Position ${position}...`);
            setMyTurn(false); // Bloquer immédiatement les clics
            loadingManager.setLoading('play-move', true);
            setError('');
        });

        // Marquer pour éviter les conflits polling
        markActionPerformed?.();

        // ✅ LOGIQUE ASYNC NON-BLOQUANTE
        try {
            const result = await gameClient.makeMove(
                currentSession.sessionId,
                currentSession.playerId,
                position
            );

            if (result.success) {
                // ✅ CAS DE SUCCÈS - TRAITEMENT IMMÉDIAT
                batch(() => {
                    const parsedState = result.newGameState ? JSON.parse(result.newGameState) : {};
                    updatePlateauTiles(parsedState);

                    setStatusMessage(`✅ Position ${position}! +${result.pointsEarned} pts`);
                    loadingManager.setLoading('play-move', false);
                });

                // ✅ MCTS en différé pour ne pas bloquer l'UI
                if (result.mctsResponse && result.mctsResponse !== "{}") {
                    setTimeout(() => {
                        try {
                            const mctsData = JSON.parse(result.mctsResponse);
                            const mctsMessage = `🤖 MCTS: position ${mctsData.position}`;
                            batch(() => {
                                setMctsLastMove(mctsMessage);
                                setStatusMessage(mctsMessage);
                            });
                        } catch (e) {
                            setMctsLastMove('🤖 MCTS a joué');
                        }
                    }, 500);
                }

                // ✅ Tour suivant en différé
                if (!result.isGameOver) {
                    setTimeout(() => {
                        startGameTurn();
                    }, 2000);
                }

                return; // ✅ SORTIR ICI - SUCCÈS TRAITÉ
            } else {
                // ✅ CAS D'ÉCHEC SERVEUR (result.success = false)
                console.log('❌ ÉCHEC SERVEUR:', result.error);
                batch(() => {
                    setMyTurn(true); // Rollback - rendre le tour
                    loadingManager.setLoading('play-move', false);
                    setError(result.error || 'Mouvement refusé');
                    setStatusMessage(`❌ ${result.error || 'Mouvement refusé'}`);
                });
                return; // ✅ SORTIR ICI - ÉCHEC TRAITÉ
            }

        } catch (error) {
            // ✅ CAS D'EXCEPTION RÉSEAU (vraie erreur technique)

            batch(() => {
                setMyTurn(true); // Rollback - rendre le tour
                loadingManager.setLoading('play-move', false);
                setError('Erreur réseau');
                setStatusMessage('💥 Problème de connexion - Réessayez');
            });
        }
    };

    /**
     * Créer une nouvelle session
     */
    const createSession = async (
        playerName: () => string,
        setSession: (session: Session) => void,
        setGameState: (state: GameState) => void,
        convertSessionState: (sessionState: any) => GameState
    ) => {
        if (!playerName().trim()) {
            setError('Veuillez entrer votre nom');
            return;
        }

        loadingManager.setLoading('create-session', true);
        setError('');

        const result = await gameClient.createSession(playerName());

        if (result.success) {
            // Validation des données de session
            if (!result.sessionId || !result.playerId || !result.sessionCode) {
                const error = `Données de session manquantes: sessionId=${result.sessionId}, playerId=${result.playerId}, sessionCode=${result.sessionCode}`;
                setError(error);
                loadingManager.setLoading('create-session', false);
                return;
            }


            setSession({
                playerId: result.playerId,
                sessionCode: result.sessionCode,
                sessionId: result.sessionId
            });

            if (result.sessionState) {
                setGameState(convertSessionState(result.sessionState));
            } else {
                setGameState({
                    sessionCode: result.sessionCode,
                    state: 0, // SessionState.WAITING
                    players: [{
                        id: result.playerId,
                        name: playerName(),
                        score: 0,
                        isReady: true,
                        isConnected: true,
                        joinedAt: Date.now().toString()
                    }],
                    boardState: "{}"
                });
            }

            setStatusMessage(`Session créée ! Code: ${result.sessionCode}`);
        } else {
            setError(result.error || 'Erreur lors de la création');
        }

        loadingManager.setLoading('create-session', false);
    };

    /**
     * Rejoindre une session existante
     */
    const joinSession = async (
        playerName: () => string,
        sessionCode: () => string,
        setSession: (session: Session) => void,
        setGameState: (state: GameState) => void,
        convertSessionState: (sessionState: any) => GameState
    ) => {
        if (!playerName().trim() || !sessionCode().trim()) {
            setError('Veuillez entrer votre nom et le code de session');
            return;
        }

        loadingManager.setLoading('join-session', true);
        setError('');

        const result = await gameClient.joinSession(sessionCode(), playerName());

        if (result.success) {
            // Validation des données de session
            if (!result.sessionId || !result.playerId || !result.sessionCode) {
                const error = `Données de session manquantes: sessionId=${result.sessionId}, playerId=${result.playerId}, sessionCode=${result.sessionCode}`;
                setError(error);
                loadingManager.setLoading('join-session', false);
                return;
            }


            setSession({
                playerId: result.playerId,
                sessionCode: result.sessionCode,
                sessionId: result.sessionId
            });

            if (result.sessionState) {
                setGameState(convertSessionState(result.sessionState));
            } else {
                setGameState({
                    sessionCode: result.sessionCode,
                    state: 0, // SessionState.WAITING
                    players: [{
                        id: result.playerId,
                        name: playerName(),
                        score: 0,
                        isReady: false,
                        isConnected: true,
                        joinedAt: Date.now().toString()
                    }],
                    boardState: "{}"
                });
            }

            setStatusMessage(`Rejoint la session ${result.sessionCode}`);
        } else {
            setError(result.error || 'Erreur lors du join');
        }

        loadingManager.setLoading('join-session', false);
    };

    /**
     * Définir le joueur comme prêt
     */
    const setReady = async (
        setGameState: (state: GameState | ((prev: GameState | null) => GameState | null)) => void
    ) => {
        const currentSession = session();
        if (!currentSession) {
            return;
        }

        // Validation renforcée
        const sessionId = currentSession.sessionId;
        const playerId = currentSession.playerId;


        if (!sessionId || !playerId) {
            const error = `Données manquantes: sessionId=${sessionId}, playerId=${playerId}`;
            setError(error);
            return;
        }

        loadingManager.setLoading('set-ready', true);

        const result = await gameClient.setPlayerReady(sessionId, playerId);

        if (result.success) {
            setGameState(prev => {
                if (!prev) return null;
                return {
                    ...prev,
                    players: prev.players.map(p =>
                        p.id === currentSession.playerId
                            ? { ...p, isReady: true }
                            : p
                    )
                };
            });

            setStatusMessage('Vous êtes maintenant prêt !');

            if (result.gameStarted) {
                setGameState(prev => prev ? { ...prev, state: 1 } : null); // SessionState.IN_PROGRESS
                setStatusMessage('La partie commence !');
            }
        } else {
            setError(result.error || 'Erreur');
        }

        loadingManager.setLoading('set-ready', false);
    };

    /**
     * Quitter la session
     */
    const leaveSession = async (resetSession: () => void) => {
        const currentSession = session();
        if (currentSession) {
            await gameClient.leaveSession(currentSession.sessionId, currentSession.playerId);
        }

        resetSession();
    };

    return {
        startGameTurn,
        playMove,
        createSession,
        joinSession,
        setReady,
        leaveSession
    };
};