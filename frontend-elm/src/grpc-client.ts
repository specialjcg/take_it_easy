// grpc-client.ts - Client gRPC-Web pour le frontend Elm
// Ce fichier sera bundlé en JavaScript pur

import { GrpcWebFetchTransport } from '@protobuf-ts/grpcweb-transport';
import { SessionServiceClient } from './generated/session_service.client';
import { GameServiceClient } from './generated/game_service.client';
import {
    CreateSessionRequest,
    JoinSessionRequest,
    SetReadyRequest,
    GetSessionStateRequest
} from './generated/session_service';
import {
    MakeMoveRequest,
    StartTurnRequest,
    GetGameStateRequest,
    GetAiMoveRequest
} from './generated/game_service';

// Auto-detect environment
const IS_PRODUCTION = typeof window !== 'undefined' &&
    window.location.hostname !== 'localhost' &&
    window.location.hostname !== '127.0.0.1';
const GRPC_WEB_URL = IS_PRODUCTION
    ? `${window.location.protocol}//${window.location.host}`  // Production: nginx proxy
    : 'http://localhost:50052';  // Development: direct gRPC-Web

class GrpcClient {
    private sessionClient: SessionServiceClient;
    private gameClient: GameServiceClient;

    constructor() {
        const transport = new GrpcWebFetchTransport({
            baseUrl: GRPC_WEB_URL,
            fetchInit: { mode: 'cors', credentials: 'omit' },
            format: "text",
            timeout: 30000
        });

        this.sessionClient = new SessionServiceClient(transport);
        this.gameClient = new GameServiceClient(transport);
    }

    async createSession(playerName: string, gameMode: string) {
        try {
            const request: CreateSessionRequest = {
                playerName,
                maxPlayers: 4,
                gameMode
            };

            const { response } = await this.sessionClient.createSession(request);

            if (response.result.oneofKind === "success") {
                const success = response.result.success;
                return {
                    success: true,
                    sessionCode: success.sessionCode,
                    sessionId: success.sessionId,
                    playerId: success.playerId,
                    sessionState: success.player ? {
                        sessionId: success.sessionId,
                        players: [success.player],
                        state: 0
                    } : undefined
                };
            } else if (response.result.oneofKind === "error") {
                return { success: false, error: response.result.error.message };
            }
            return { success: false, error: "Réponse invalide" };
        } catch (error: any) {
            return { success: false, error: error.message || 'Erreur réseau' };
        }
    }

    async joinSession(sessionCode: string, playerName: string) {
        try {
            const request: JoinSessionRequest = {
                sessionCode: sessionCode.toUpperCase(),
                playerName
            };

            const { response } = await this.sessionClient.joinSession(request);

            if (response.result.oneofKind === "success") {
                const success = response.result.success;
                return {
                    success: true,
                    sessionCode,
                    sessionId: success.sessionId,
                    playerId: success.playerId,
                    sessionState: success.gameState
                };
            } else if (response.result.oneofKind === "error") {
                return { success: false, error: response.result.error.message };
            }
            return { success: false, error: "Réponse invalide" };
        } catch (error: any) {
            return { success: false, error: error.message || 'Erreur réseau' };
        }
    }

    async setReady(sessionId: string, playerId: string) {
        try {
            const request: SetReadyRequest = { sessionId, playerId, ready: true };
            const { response } = await this.sessionClient.setReady(request);

            if (response.success) {
                return { success: true, gameStarted: response.gameStarted };
            } else if (response.error) {
                return { success: false, error: response.error.message };
            }
            return { success: false, error: "Échec" };
        } catch (error: any) {
            return { success: false, error: error.message || 'Erreur réseau' };
        }
    }

    async getSessionState(sessionId: string) {
        try {
            const request: GetSessionStateRequest = { sessionId };
            const { response } = await this.sessionClient.getSessionState(request);

            if (response.gameState) {
                return { success: true, sessionState: response.gameState };
            } else if (response.error) {
                return { success: false, error: response.error.message };
            }
            return { success: false, error: "Aucun état" };
        } catch (error: any) {
            return { success: false, error: error.message || 'Erreur réseau' };
        }
    }

    async startTurn(sessionId: string, forcedTile?: string) {
        try {
            // Support du mode Jeu Réel avec tuile forcée
            const request: any = { sessionId };
            if (forcedTile) {
                request.forcedTile = forcedTile;
            }
            const { response } = await this.gameClient.startTurn(request);

            if (response.success) {
                return {
                    success: true,
                    announcedTile: response.announcedTile,
                    tileImage: response.tileImage,
                    turnNumber: response.turnNumber,
                    waitingForPlayers: response.waitingForPlayers,
                    gameState: response.gameState
                };
            } else if (response.error) {
                return { success: false, error: response.error.message };
            }
            return { success: false, error: "Échec" };
        } catch (error: any) {
            return { success: false, error: error.message || 'Erreur réseau' };
        }
    }

    // Mode Jeu Réel: obtenir la recommandation IA pour une tuile
    async getAiMove(tileCode: string, boardState: string[], availablePositions: number[], turnNumber: number) {
        try {
            const request: GetAiMoveRequest = {
                tileCode,
                boardState,
                availablePositions,
                turnNumber
            };
            const { response } = await this.gameClient.getAiMove(request);

            if (response.success) {
                return {
                    success: true,
                    recommendedPosition: response.recommendedPosition
                };
            } else if (response.error) {
                return { success: false, error: response.error.message };
            }
            return { success: false, error: "Échec" };
        } catch (error: any) {
            // Si getAiMove n'existe pas dans le client généré, on retourne une erreur gracieuse
            console.warn('getAiMove non disponible:', error);
            return { success: false, error: 'AI non disponible - utilisez le mode avec session' };
        }
    }

    async makeMove(sessionId: string, playerId: string, position: number) {
        try {
            const request: MakeMoveRequest = {
                sessionId,
                playerId,
                moveData: `{"position":${position}}`,
                timestamp: 0n
            };

            const { response } = await this.gameClient.makeMove(request);

            if (response.result.oneofKind === "success") {
                const success = response.result.success;
                return {
                    success: true,
                    pointsEarned: success.pointsEarned,
                    mctsResponse: success.mctsResponse,
                    isGameOver: success.isGameOver,
                    newGameState: success.newGameState || {}
                };
            } else if (response.result.oneofKind === "error") {
                return { success: false, error: response.result.error.message };
            }
            return { success: false, error: "Échec" };
        } catch (error: any) {
            return { success: false, error: error.message || 'Erreur réseau' };
        }
    }

    async getGameState(sessionId: string) {
        try {
            const request: GetGameStateRequest = { sessionId };
            const { response } = await this.gameClient.getGameState(request);

            if (response.success) {
                return {
                    success: true,
                    gameState: response.gameState,
                    currentTile: response.currentTile,
                    currentTileImage: response.currentTileImage,
                    currentTurn: response.currentTurn,
                    isGameFinished: response.isGameFinished,
                    finalScores: response.finalScores
                };
            } else if (response.error) {
                return { success: false, error: response.error.message };
            }
            return { success: false, error: "Échec" };
        } catch (error: any) {
            return { success: false, error: error.message || 'Erreur réseau' };
        }
    }
}

// Export global pour utilisation dans ports.js
(window as any).grpcClient = new GrpcClient();
