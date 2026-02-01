// grpc-client.ts - Client gRPC-Web pour le frontend Elm
// Ce fichier sera bundlé en JavaScript pur

import { GrpcWebFetchTransport } from '@protobuf-ts/grpcweb-transport';
import { SessionServiceClient } from '../../frontend/src/generated/session_service.client';
import { GameServiceClient } from '../../frontend/src/generated/game_service.client';
import {
    CreateSessionRequest,
    JoinSessionRequest,
    SetReadyRequest,
    GetSessionStateRequest
} from '../../frontend/src/generated/session_service';
import {
    MakeMoveRequest,
    StartTurnRequest,
    GetGameStateRequest
} from '../../frontend/src/generated/game_service';

const GRPC_WEB_URL = 'http://localhost:50052';

class GrpcClient {
    private sessionClient: SessionServiceClient;
    private gameClient: GameServiceClient;

    constructor() {
        const transport = new GrpcWebFetchTransport({
            baseUrl: GRPC_WEB_URL,
            fetchInit: { mode: 'cors', credentials: 'omit' },
            format: "binary",
            timeout: 10000,
            meta: {
                'content-type': 'application/grpc-web+proto',
                'accept': 'application/grpc-web+proto'
            }
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

    async startTurn(sessionId: string) {
        try {
            const request: StartTurnRequest = { sessionId };
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

    async makeMove(sessionId: string, playerId: string, position: number) {
        try {
            const request: MakeMoveRequest = {
                sessionId,
                playerId,
                moveData: `{"position":${position}}`,
                timestamp: BigInt(Date.now())
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
