// src/services/GameClient.ts - Version complète avec corrections

import { GrpcWebFetchTransport } from '@protobuf-ts/grpcweb-transport';
import { SessionServiceClient } from '../generated/session_service.client';
import { GameServiceClient } from '../generated/game_service.client';
import {
    CreateSessionRequest,
    JoinSessionRequest,
    SetReadyRequest,
    GetSessionStateRequest
} from '../generated/session_service';
import {
    MakeMoveRequest,
    GetAvailableMovesRequest,
    StartTurnRequest,
    GetGameStateRequest
} from '../generated/game_service';
import type { GameState } from '../generated/common';

export class GameClient {
    private sessionClient: SessionServiceClient;
    private gameClient: GameServiceClient;
    private transport: GrpcWebFetchTransport;
    private debugEnabled = false; // ✅ DEBUG DÉSACTIVÉ - Trop de spam dans les logs

    constructor() {
        this.transport = new GrpcWebFetchTransport({
            baseUrl: 'http://localhost:50051',
            fetchInit: {
                mode: 'cors',
                credentials: 'omit'
            },
            format: "binary",
            compress: false,
            timeout: 10000,
            meta: {
                'content-type': 'application/grpc-web+proto',
                'accept': 'application/grpc-web+proto'
            }
        });

        this.sessionClient = new SessionServiceClient(this.transport);
        this.gameClient = new GameServiceClient(this.transport);

        this.debugLog('🔌 GameClient initialisé (version complète)');
    }

    // 🔧 AJOUT: Helper de debug
    private debugLog(message: string, data?: any) {
        if (this.debugEnabled) {
            console.log(`🔍 GameClient: ${message}`, data || '');
        }
    }

    // ============================================================================
    // MÉTHODES GESTION DE SESSION (avec debug amélioré)
    // ============================================================================

    async createSession(playerName: string, gameMode: string = "multiplayer") {
        this.debugLog('📝 createSession DÉBUT', { playerName, gameMode });

        try {
            const request: CreateSessionRequest = {
                playerName: playerName,
                maxPlayers: 4,
                gameMode: gameMode
            };

            this.debugLog('📤 Envoi createSession request', request);
            const { response } = await this.sessionClient.createSession(request);
            this.debugLog('📥 createSession response reçue', response);

            if (response.result.oneofKind === "success") {
                const success = response.result.success;
                this.debugLog('✅ createSession SUCCESS', {
                    sessionCode: success.sessionCode,
                    sessionId: success.sessionId,
                    playerId: success.playerId
                });

                return {
                    success: true,
                    sessionCode: success.sessionCode,
                    sessionId: success.sessionId,
                    playerId: success.playerId,
                    sessionState: success.player ? {
                        sessionId: success.sessionId,
                        players: [success.player],
                        currentPlayerId: success.playerId,
                        state: 0,
                        boardState: "{}",
                        turnNumber: 0
                    } as GameState : undefined
                };
            } else if (response.result.oneofKind === "error") {
                this.debugLog('❌ createSession ERROR', response.result.error);
                return {
                    success: false,
                    error: response.result.error.message
                };
            }

            this.debugLog('❌ createSession INVALID RESPONSE');
            return {
                success: false,
                error: "Réponse invalide du serveur"
            };
        } catch (error) {
            this.debugLog('💥 createSession EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }

    async joinSession(sessionCode: string, playerName: string) {
        this.debugLog('🚪 joinSession DÉBUT', { sessionCode, playerName });

        try {
            const request: JoinSessionRequest = {
                sessionCode: sessionCode.toUpperCase(),
                playerName: playerName
            };

            this.debugLog('📤 Envoi joinSession request', request);
            const { response } = await this.sessionClient.joinSession(request);
            this.debugLog('📥 joinSession response reçue', response);

            if (response.result.oneofKind === "success") {
                const success = response.result.success;
                this.debugLog('✅ joinSession SUCCESS', {
                    sessionId: success.sessionId,
                    playerId: success.playerId
                });

                return {
                    success: true,
                    sessionCode: sessionCode,
                    sessionId: success.sessionId,
                    playerId: success.playerId,
                    sessionState: success.gameState
                };
            } else if (response.result.oneofKind === "error") {
                this.debugLog('❌ joinSession ERROR', response.result.error);
                return {
                    success: false,
                    error: response.result.error.message
                };
            }

            this.debugLog('❌ joinSession INVALID RESPONSE');
            return {
                success: false,
                error: "Réponse invalide du serveur"
            };
        } catch (error) {
            this.debugLog('💥 joinSession EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }

    // 🔧 CORRIGÉ: setPlayerReady avec validation renforcée
    async setPlayerReady(sessionId: string, playerId: string) {
        this.debugLog('⚡ setPlayerReady DÉBUT - VALIDATION');
        this.debugLog('  📋 sessionId', sessionId);
        this.debugLog('  📋 playerId', playerId);

        // 🔍 Validation ultra-stricte (comme dans la version debug)
        if (!sessionId) {
            this.debugLog('❌ sessionId is falsy', sessionId);
            return { success: false, error: 'sessionId is required' };
        }
        if (!playerId) {
            this.debugLog('❌ playerId is falsy', playerId);
            return { success: false, error: 'playerId is required' };
        }
        if (typeof sessionId !== 'string') {
            this.debugLog('❌ sessionId is not string', typeof sessionId);
            return { success: false, error: 'sessionId must be string' };
        }
        if (typeof playerId !== 'string') {
            this.debugLog('❌ playerId is not string', typeof playerId);
            return { success: false, error: 'playerId must be string' };
        }

        try {
            const request: SetReadyRequest = {
                sessionId: sessionId,
                playerId: playerId,
                ready: true
            };

            this.debugLog('📤 Envoi setReady request', request);
            const { response } = await this.sessionClient.setReady(request);
            this.debugLog('📥 setReady response reçue', response);

            if (response.success) {
                this.debugLog('✅ setReady SUCCESS', { gameStarted: response.gameStarted });
                return {
                    success: true,
                    gameStarted: response.gameStarted
                };
            } else if (response.error) {
                this.debugLog('❌ setReady SERVER ERROR', response.error);
                return {
                    success: false,
                    error: response.error.message
                };
            }

            this.debugLog('❌ setReady INVALID RESPONSE FORMAT');
            return {
                success: false,
                error: "Échec de la requête"
            };
        } catch (error) {
            this.debugLog('💥 setReady EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }

    async getSessionState(sessionId: string) {
        // Log léger pour éviter le spam pendant le polling
        this.debugLog('📊 getSessionState', { sessionId: sessionId?.substring(0, 8) + '...' });

        try {
            const request: GetSessionStateRequest = {
                sessionId: sessionId
            };

            const { response } = await this.sessionClient.getSessionState(request);

            if (response.gameState) {
                return {
                    success: true,
                    sessionState: response.gameState
                };
            } else if (response.error) {
                this.debugLog('❌ getSessionState ERROR', response.error);
                return {
                    success: false,
                    error: response.error.message
                };
            }

            return {
                success: false,
                error: "Aucun état de session retourné"
            };
        } catch (error) {
            this.debugLog('💥 getSessionState EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }

    async leaveSession(sessionId: string, playerId: string) {
        this.debugLog('🚪 leaveSession', { sessionId, playerId });
        try {
            return { success: true };
        } catch (error) {
            this.debugLog('💥 leaveSession EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }

    // ============================================================================
    // 🎲 NOUVELLES MÉTHODES GAMEPLAY - gRPC UNIQUEMENT (avec debug)
    // ============================================================================

    // 🎲 Démarrer un nouveau tour (tire une tuile aléatoire)
    async startNewTurn(sessionId: string) {
        this.debugLog('🎲 startNewTurn DÉBUT', { sessionId });

        try {
            const request: StartTurnRequest = {
                sessionId: sessionId
            };

            this.debugLog('📤 Envoi startTurn request', request);
            const { response } = await this.gameClient.startTurn(request);
            this.debugLog('📥 startTurn response reçue', response);

            if (response.success) {
                this.debugLog('✅ startTurn SUCCESS', {
                    announcedTile: response.announcedTile,
                    turnNumber: response.turnNumber
                });

                return {
                    success: true,
                    announcedTile: response.announcedTile,
                    tileImage: response.tileImage,
                    turnNumber: response.turnNumber,
                    waitingForPlayers: response.waitingForPlayers,
                    gameState: response.gameState
                };
            } else if (response.error) {
                this.debugLog('❌ startTurn ERROR', response.error);
                return {
                    success: false,
                    error: response.error.message
                };
            }

            return {
                success: false,
                error: "Échec du démarrage du tour"
            };
        } catch (error) {
            this.debugLog('💥 startTurn EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }

    // 🎯 Jouer un mouvement (position sur le plateau)
    async makeMove(sessionId: string, playerId: string, position: number) {
        this.debugLog('🎯 makeMove DÉBUT', { sessionId, playerId, position });

        try {
            const request: MakeMoveRequest = {
                sessionId: sessionId,
                playerId: playerId,
                moveData: `{"position":${position}}`,
                timestamp: BigInt(Date.now())
            };

            this.debugLog('📤 Envoi makeMove request', request);
            const { response } = await this.gameClient.makeMove(request);
            this.debugLog('📥 makeMove response reçue', response);

            if (response.result.oneofKind === "success") {
                const success = response.result.success;
                this.debugLog('✅ makeMove SUCCESS', {
                    pointsEarned: success.pointsEarned,
                    isGameOver: success.isGameOver
                });

                return {
                    success: true,
                    pointsEarned: success.pointsEarned,
                    mctsResponse: success.mctsResponse,
                    turnCompleted: true,
                    isGameOver: success.isGameOver,
                    finalScores: "{}",
                    newGameState: success.newGameState || {}
                };
            } else if (response.result.oneofKind === "error") {
                this.debugLog('❌ makeMove ERROR', response.result.error);
                return {
                    success: false,
                    error: response.result.error.message
                };
            }

            return {
                success: false,
                error: "Échec du mouvement"
            };
        } catch (error) {
            this.debugLog('💥 makeMove EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }
// Ajoutez cette méthode dans votre classe GameClient
    private safeStringify(obj: any): string {
        if (!obj) return '{}';

        try {
            return JSON.stringify(obj, (key, value) => {
                // Convertir BigInt en string pour éviter l'erreur
                if (typeof value === 'bigint') {
                    return value.toString();
                }
                return value;
            });
        } catch (error) {
            // Si ça plante encore, conversion simple en string
            return String(obj);
        }
    }
    // 📊 Obtenir l'état du gameplay
    async getGameplayState(sessionId: string) {
        this.debugLog('📊 getGameplayState', { sessionId });

        try {
            const request: GetGameStateRequest = {
                sessionId: sessionId
            };

            const { response } = await this.gameClient.getGameState(request);

            if (response.success) {
                return {
                    success: true,
                    gameState: response.gameState,
                    currentTile: response.currentTile,
                    currentTileImage: response.currentTileImage, // ✅ NOUVEAU CHAMP!
                    currentTurn: response.currentTurn,
                    waitingForPlayers: response.waitingForPlayers,
                    isGameFinished: response.isGameFinished,
                    finalScores: response.finalScores
                };
            } else if (response.error) {
                this.debugLog('❌ getGameplayState ERROR', response.error);
                return {
                    success: false,
                    error: response.error.message
                };
            }

            return {
                success: false,
                error: "Échec de récupération de l'état"
            };
        } catch (error) {
            this.debugLog('💥 getGameplayState EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }
    async getGameState(sessionId: string) {
        this.debugLog('🎮 getGameState DIRECT', { sessionId });

        try {
            const request: GetGameStateRequest = {
                sessionId: sessionId
            };

            const { response } = await this.gameClient.getGameState(request);

            if (response.success) {
                this.debugLog('✅ getGameState SUCCESS', {
                    currentTile: response.currentTile,
                    currentTileImage: response.currentTileImage, // ✅ Log pour debug
                    currentTurn: response.currentTurn,
                    isGameFinished: response.isGameFinished
                });

                return {
                    success: true,
                    gameState: response.gameState,
                    currentTile: response.currentTile,
                    currentTileImage: response.currentTileImage, // ✅ EXPOSÉ!
                    currentTurn: response.currentTurn,
                    waitingForPlayers: response.waitingForPlayers,
                    isGameFinished: response.isGameFinished,
                    finalScores: response.finalScores
                };
            } else if (response.error) {
                this.debugLog('❌ getGameState ERROR', response.error);
                return {
                    success: false,
                    error: response.error.message
                };
            }

            return {
                success: false,
                error: "Échec de récupération de l'état de jeu"
            };
        } catch (error) {
            this.debugLog('💥 getGameState EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }
    // 🎯 Obtenir les mouvements disponibles
    async getAvailableMoves(sessionId: string, playerId: string) {
        this.debugLog('🎯 getAvailableMoves', { sessionId, playerId });

        try {
            const request: GetAvailableMovesRequest = {
                sessionId: sessionId,
                playerId: playerId
            };

            const { response } = await this.gameClient.getAvailableMoves(request);

            if (!response.error) {
                return {
                    success: true,
                    availableMoves: response.availableMoves
                };
            } else {
                this.debugLog('❌ getAvailableMoves ERROR', response.error);
                return {
                    success: false,
                    error: response.error.message
                };
            }
        } catch (error) {
            this.debugLog('💥 getAvailableMoves EXCEPTION', error);
            return {
                success: false,
                error: this.extractErrorMessage(error)
            };
        }
    }

    // ============================================================================
    // MÉTHODES UTILITAIRES (améliorées)
    // ============================================================================

    private extractErrorMessage(error: any): string {
        this.debugLog('🔍 extractErrorMessage input', error);

        if (error?.message) {
            return error.message;
        }
        if (typeof error === 'string') {
            return error;
        }
        return 'Erreur de connexion au serveur';
    }

    // 🔧 NOUVEAU: Contrôle du debug
    setDebugEnabled(enabled: boolean) {
        this.debugEnabled = enabled;
        this.debugLog(`🔧 Debug ${enabled ? 'activé' : 'désactivé'}`);
    }

    // 🔧 NOUVEAU: Test de connectivité
    async testConnection() {
        this.debugLog('🔧 Test de connectivité');
        try {
            // Test simple avec une session inexistante
            await this.getSessionState('test-connection-123');
            return { success: true, note: 'Server reachable' };
        } catch (error: any) {
            // Si on reçoit "SESSION_NOT_FOUND", c'est que la connexion fonctionne
            if (error.message && error.message.includes('SESSION_NOT_FOUND')) {
                return { success: true, note: 'Server reachable (test session not found)' };
            }
            return { success: false, error: this.extractErrorMessage(error) };
        }
    }

    dispose() {
        this.debugLog('🔌 GameClient disposed');
    }
}

// Instance singleton
export const gameClient = new GameClient();