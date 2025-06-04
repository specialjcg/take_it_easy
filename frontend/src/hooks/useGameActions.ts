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
    setLoading: (loading: boolean) => void,
    setError: (error: string) => void,
    setStatusMessage: (message: string) => void,
    setCurrentTile: (tile: string | null) => void,
    setCurrentTileImage: (image: string | null) => void,
    setCurrentTurnNumber: (turn: number) => void,
    setIsGameStarted: (started: boolean) => void,
    setMyTurn: (turn: boolean) => void,
    setMctsLastMove: (move: string) => void,
    updatePlateauTiles: (gameState: any) => void,
    addDebugLog: (message: string) => void,
) => {

    /**
     * Démarrer un nouveau tour (tire une tuile aléatoire)
     */
        // ✅ SOLUTION 2: startGameTurn optimisé
    const startGameTurn = async () => {
            const currentSession = session();
            if (!currentSession) return;

            // ✅ BATCH: État de chargement
            batch(() => {
                setLoading(true);
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
                        setLoading(false);
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
                        setLoading(false);
                    });
                }
            } catch (error) {
                batch(() => {
                    setError('Erreur connexion');
                    setLoading(false);
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
            setLoading(true);
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

            // ✅ BATCH 2: Mise à jour résultat (1 seul re-render)
            batch(() => {
                if (result.success) {
                    setStatusMessage(`✅ Position ${position}! +${result.pointsEarned} pts`);
                    setLoading(false);

                    // État plateau mis à jour en arrière-plan (pas de re-render immédiat)
                    if (result.newGameState) {
                        queueMicrotask(() => {
                            const parsedState = JSON.parse(result.newGameState);
                            updatePlateauTiles(parsedState);
                        });
                    }

                    // MCTS en différé pour ne pas bloquer l'UI
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
                        }, 500); // Délai pour voir la confirmation du joueur
                    }

                    // Tour suivant en différé
                    if (!result.isGameOver) {
                        setTimeout(() => {
                            startGameTurn();
                        }, 2000);
                    }
                } else {
                    // ROLLBACK en cas d'échec
                    setMyTurn(true);
                    setLoading(false);
                    setError(result.error || 'Mouvement refusé');
                    setStatusMessage(`❌ ${result.error}`);
                }
            });

        } catch (error) {
            // ✅ BATCH 3: Gestion d'erreur (1 seul re-render)
            batch(() => {
                setMyTurn(true);
                setLoading(false);
                setError('Erreur réseau');
                setStatusMessage('💥 Réessayez');
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

        setLoading(true);
        setError('');
        addDebugLog(`🎯 Création session pour ${playerName()}`);

        const result = await gameClient.createSession(playerName());

        if (result.success) {
            // Validation des données de session
            if (!result.sessionId || !result.playerId || !result.sessionCode) {
                const error = `Données de session manquantes: sessionId=${result.sessionId}, playerId=${result.playerId}, sessionCode=${result.sessionCode}`;
                setError(error);
                addDebugLog(`❌ ${error}`);
                setLoading(false);
                return;
            }

            addDebugLog(`✅ Session créée avec succès: sessionId=${result.sessionId}, playerId=${result.playerId}`);

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
            addDebugLog(`✅ Session créée: ${result.sessionCode}`);
        } else {
            setError(result.error || 'Erreur lors de la création');
            addDebugLog(`❌ Échec création: ${result.error}`);
        }

        setLoading(false);
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

        setLoading(true);
        setError('');
        addDebugLog(`🚪 Join session: ${sessionCode()} par ${playerName()}`);

        const result = await gameClient.joinSession(sessionCode(), playerName());

        if (result.success) {
            // Validation des données de session
            if (!result.sessionId || !result.playerId || !result.sessionCode) {
                const error = `Données de session manquantes: sessionId=${result.sessionId}, playerId=${result.playerId}, sessionCode=${result.sessionCode}`;
                setError(error);
                addDebugLog(`❌ ${error}`);
                setLoading(false);
                return;
            }

            addDebugLog(`✅ Session jointe avec succès: sessionId=${result.sessionId}, playerId=${result.playerId}`);

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
            addDebugLog(`✅ Session jointe: ${result.sessionCode}`);
        } else {
            setError(result.error || 'Erreur lors du join');
            addDebugLog(`❌ Échec join: ${result.error}`);
        }

        setLoading(false);
    };

    /**
     * Définir le joueur comme prêt
     */
    const setReady = async (
        setGameState: (state: GameState | ((prev: GameState | null) => GameState | null)) => void
    ) => {
        const currentSession = session();
        if (!currentSession) {
            addDebugLog('❌ Pas de session active');
            return;
        }

        // Validation renforcée
        const sessionId = currentSession.sessionId;
        const playerId = currentSession.playerId;

        addDebugLog(`⚡ SET_READY: sessionId="${sessionId}", playerId="${playerId}"`);

        if (!sessionId || !playerId) {
            const error = `Données manquantes: sessionId=${sessionId}, playerId=${playerId}`;
            setError(error);
            addDebugLog(`❌ ${error}`);
            return;
        }

        setLoading(true);

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
            addDebugLog(`✅ Ready défini - Game Started: ${result.gameStarted}`);

            if (result.gameStarted) {
                setGameState(prev => prev ? { ...prev, state: 1 } : null); // SessionState.IN_PROGRESS
                setStatusMessage('La partie commence !');
                addDebugLog('🎮 Jeu démarré');
            }
        } else {
            setError(result.error || 'Erreur');
            addDebugLog(`❌ Échec setReady: ${result.error}`);
        }

        setLoading(false);
    };

    /**
     * Quitter la session
     */
    const leaveSession = async (resetSession: () => void) => {
        const currentSession = session();
        if (currentSession) {
            await gameClient.leaveSession(currentSession.sessionId, currentSession.playerId);
            addDebugLog(`🚪 Session quittée: ${currentSession.sessionCode}`);
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