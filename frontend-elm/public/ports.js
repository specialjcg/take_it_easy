/**
 * ports.js - JavaScript interop for Elm
 * Uses the bundled gRPC-Web client for game communication
 * and direct fetch for Auth API
 */

// Configuration
const AUTH_API_BASE = 'http://localhost:51051/auth';

// LocalStorage keys
const TOKEN_KEY = 'auth_token';
const USER_KEY = 'auth_user';

/**
 * Initialize ports for the Elm app
 */
function initPorts(app) {
    // Listen for messages from Elm
    app.ports.sendToJs.subscribe(async (message) => {
        console.log('Elm -> JS:', message);

        try {
            switch (message.type) {
                // ========== AUTH ==========
                case 'login':
                    await handleLogin(app, message.email, message.password);
                    break;
                case 'register':
                    await handleRegister(app, message.email, message.username, message.password);
                    break;
                case 'logout':
                    handleLogout(app);
                    break;
                case 'checkAuth':
                    await handleCheckAuth(app);
                    break;

                // ========== SESSION (via gRPC) ==========
                case 'createSession':
                    await handleCreateSession(app, message.playerName, message.gameMode);
                    break;
                case 'joinSession':
                    await handleJoinSession(app, message.sessionCode, message.playerName);
                    break;
                case 'leaveSession':
                    await handleLeaveSession(app, message.sessionId, message.playerId);
                    break;
                case 'setReady':
                    await handleSetReady(app, message.sessionId, message.playerId);
                    break;

                // ========== GAMEPLAY (via gRPC) ==========
                case 'startTurn':
                    await handleStartTurn(app, message.sessionId);
                    break;
                case 'playMove':
                    await handlePlayMove(app, message.sessionId, message.playerId, message.position);
                    break;

                default:
                    console.warn('Unknown message type:', message.type);
            }
        } catch (error) {
            console.error('Error handling message:', error);
        }
    });
}

// ============================================================================
// AUTH HANDLERS (direct fetch to REST API)
// ============================================================================

async function handleLogin(app, email, password) {
    try {
        const response = await fetch(`${AUTH_API_BASE}/login`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email, password })
        });

        if (response.ok) {
            const data = await response.json();
            saveAuth(data.token, data.user);
            app.ports.receiveFromJs.send({
                type: 'loginSuccess',
                user: {
                    id: data.user.id,
                    email: data.user.email,
                    username: data.user.username,
                    emailVerified: data.user.email_verified || false
                },
                token: data.token
            });
        } else {
            const error = await response.json();
            app.ports.receiveFromJs.send({
                type: 'loginFailure',
                error: error.error || 'Erreur de connexion'
            });
        }
    } catch (e) {
        console.error('Login error:', e);
        app.ports.receiveFromJs.send({
            type: 'loginFailure',
            error: 'Erreur réseau'
        });
    }
}

async function handleRegister(app, email, username, password) {
    try {
        const response = await fetch(`${AUTH_API_BASE}/register`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email, username, password })
        });

        if (response.ok) {
            const data = await response.json();
            saveAuth(data.token, data.user);
            app.ports.receiveFromJs.send({
                type: 'registerSuccess',
                user: {
                    id: data.user.id,
                    email: data.user.email,
                    username: data.user.username,
                    emailVerified: data.user.email_verified || false
                },
                token: data.token
            });
        } else {
            const error = await response.json();
            app.ports.receiveFromJs.send({
                type: 'registerFailure',
                error: error.error || "Erreur d'inscription"
            });
        }
    } catch (e) {
        console.error('Register error:', e);
        app.ports.receiveFromJs.send({
            type: 'registerFailure',
            error: 'Erreur réseau'
        });
    }
}

function handleLogout(app) {
    clearAuth();
}

async function handleCheckAuth(app) {
    const { token, user } = loadAuth();

    if (token && user) {
        try {
            const response = await fetch(`${AUTH_API_BASE}/me`, {
                headers: { 'Authorization': `Bearer ${token}` }
            });

            if (response.ok) {
                const userData = await response.json();
                app.ports.receiveFromJs.send({
                    type: 'checkAuthSuccess',
                    user: {
                        id: userData.id,
                        email: userData.email,
                        username: userData.username,
                        emailVerified: userData.email_verified || false
                    },
                    token: token
                });
                return;
            }
        } catch (e) {
            console.error('Check auth error:', e);
        }
        clearAuth();
    }

    app.ports.receiveFromJs.send({ type: 'checkAuthFailure' });
}

// ============================================================================
// SESSION HANDLERS (via gRPC client)
// ============================================================================

async function handleCreateSession(app, playerName, gameMode) {
    if (!window.grpcClient) {
        app.ports.receiveFromJs.send({
            type: 'sessionError',
            error: 'gRPC client not loaded'
        });
        return;
    }

    try {
        const result = await window.grpcClient.createSession(playerName, gameMode);
        console.log('createSession result:', result);

        if (result.success) {
            app.ports.receiveFromJs.send({
                type: 'sessionCreated',
                session: {
                    sessionId: result.sessionId,
                    playerId: result.playerId,
                    sessionCode: result.sessionCode
                },
                gameState: parseGameState(result.sessionState)
            });
        } else {
            app.ports.receiveFromJs.send({
                type: 'sessionError',
                error: result.error || 'Erreur création session'
            });
        }
    } catch (e) {
        console.error('createSession error:', e);
        app.ports.receiveFromJs.send({
            type: 'sessionError',
            error: e.message || 'Erreur réseau'
        });
    }
}

async function handleJoinSession(app, sessionCode, playerName) {
    if (!window.grpcClient) {
        app.ports.receiveFromJs.send({
            type: 'sessionError',
            error: 'gRPC client not loaded'
        });
        return;
    }

    try {
        const result = await window.grpcClient.joinSession(sessionCode, playerName);

        if (result.success) {
            app.ports.receiveFromJs.send({
                type: 'sessionJoined',
                session: {
                    sessionId: result.sessionId,
                    playerId: result.playerId,
                    sessionCode: result.sessionCode
                },
                gameState: parseGameState(result.sessionState)
            });
        } else {
            app.ports.receiveFromJs.send({
                type: 'sessionError',
                error: result.error || 'Erreur join session'
            });
        }
    } catch (e) {
        console.error('joinSession error:', e);
        app.ports.receiveFromJs.send({
            type: 'sessionError',
            error: e.message || 'Erreur réseau'
        });
    }
}

async function handleLeaveSession(app, sessionId, playerId) {
    app.ports.receiveFromJs.send({ type: 'sessionLeft' });
}

async function handleSetReady(app, sessionId, playerId) {
    if (!window.grpcClient) {
        app.ports.receiveFromJs.send({
            type: 'sessionError',
            error: 'gRPC client not loaded'
        });
        return;
    }

    try {
        const result = await window.grpcClient.setReady(sessionId, playerId);

        if (result.success) {
            app.ports.receiveFromJs.send({
                type: 'readySet',
                gameStarted: result.gameStarted || false
            });
        } else {
            app.ports.receiveFromJs.send({
                type: 'sessionError',
                error: result.error || 'Erreur set ready'
            });
        }
    } catch (e) {
        console.error('setReady error:', e);
        app.ports.receiveFromJs.send({
            type: 'sessionError',
            error: e.message || 'Erreur réseau'
        });
    }
}

// ============================================================================
// GAMEPLAY HANDLERS (via gRPC client)
// ============================================================================

async function handleStartTurn(app, sessionId) {
    if (!window.grpcClient) {
        app.ports.receiveFromJs.send({
            type: 'gameError',
            error: 'gRPC client not loaded'
        });
        return;
    }

    try {
        const result = await window.grpcClient.startTurn(sessionId);
        console.log('startTurn result:', result);

        if (result.success) {
            let positions = [];
            let players = [];

            if (result.gameState) {
                try {
                    const gs = typeof result.gameState === 'string'
                        ? JSON.parse(result.gameState)
                        : result.gameState;

                    // Extraire les positions disponibles (joueur humain uniquement)
                    if (gs.player_plateaus) {
                        Object.entries(gs.player_plateaus).forEach(([id, plateau]) => {
                            if (plateau.available_positions && id !== 'mcts_ai') {
                                positions = plateau.available_positions;
                            }
                        });
                    }

                    // Extraire les joueurs avec leurs scores
                    if (gs.scores) {
                        Object.entries(gs.scores).forEach(([id, score]) => {
                            players.push({
                                id: id,
                                name: id === 'mcts_ai' ? 'IA' : 'Joueur',
                                score: score || 0,
                                isReady: true,
                                isConnected: true
                            });
                        });
                    }
                } catch (e) {
                    console.warn('Parse gameState error:', e);
                }
            }

            // Fix image path: ../image/X.png -> image/X.png
            const tileImage = (result.tileImage || '').replace(/^\.\.\//, '');

            app.ports.receiveFromJs.send({
                type: 'turnStarted',
                tile: result.announcedTile || '',
                tileImage: tileImage,
                turnNumber: result.turnNumber || 0,
                positions: positions,
                players: players
            });
        } else {
            app.ports.receiveFromJs.send({
                type: 'gameError',
                error: result.error || 'Erreur start turn'
            });
        }
    } catch (e) {
        console.error('startTurn error:', e);
        app.ports.receiveFromJs.send({
            type: 'gameError',
            error: e.message || 'Erreur réseau'
        });
    }
}

async function handlePlayMove(app, sessionId, playerId, position) {
    if (!window.grpcClient) {
        app.ports.receiveFromJs.send({
            type: 'gameError',
            error: 'gRPC client not loaded'
        });
        return;
    }

    try {
        const result = await window.grpcClient.makeMove(sessionId, playerId, position);
        console.log('makeMove result:', result);

        if (result.success) {
            app.ports.receiveFromJs.send({
                type: 'movePlayed',
                position: position,
                points: result.pointsEarned || 0
            });

            if (result.isGameOver) {
                const players = [];
                const plateaus = {};

                // Parse newGameState - it may have nested boardState
                let gameState = result.newGameState;
                if (typeof gameState === 'string') {
                    try {
                        gameState = JSON.parse(gameState);
                    } catch (e) {
                        console.error('Failed to parse newGameState:', e);
                        gameState = {};
                    }
                }

                // The actual game data may be in boardState (nested JSON)
                let boardData = gameState;
                if (gameState?.boardState) {
                    try {
                        boardData = typeof gameState.boardState === 'string'
                            ? JSON.parse(gameState.boardState)
                            : gameState.boardState;
                    } catch (e) {
                        console.error('Failed to parse boardState:', e);
                        boardData = {};
                    }
                }

                const scores = boardData?.scores || {};
                console.log('gameFinished boardData:', boardData);
                console.log('gameFinished scores:', scores);

                if (boardData && boardData.player_plateaus) {
                    Object.entries(boardData.player_plateaus).forEach(([id, plateau]) => {
                        players.push({
                            id: id,
                            name: id === 'mcts_ai' ? 'IA' : 'Joueur',
                            score: scores[id] || 0,
                            isReady: true,
                            isConnected: true
                        });
                        // Extract tile images for each position from 'tiles' field
                        const tileImages = [];
                        if (plateau.tiles) {
                            for (let i = 0; i < 19; i++) {
                                const tile = plateau.tiles[i];
                                // Tile is serialized as array [v1, v2, v3]
                                // Empty tiles have values 0,0,0
                                if (tile && (tile[0] !== 0 || tile[1] !== 0 || tile[2] !== 0)) {
                                    // Format: "image/XYZ.png" from tile values
                                    tileImages.push(`image/${tile[0]}${tile[1]}${tile[2]}.png`);
                                } else {
                                    tileImages.push('');
                                }
                            }
                        }
                        plateaus[id] = tileImages;
                    });
                }
                console.log('gameFinished plateaus:', plateaus);
                console.log('gameFinished players:', players);
                app.ports.receiveFromJs.send({
                    type: 'gameFinished',
                    players: players,
                    plateaus: plateaus
                });
            }
        } else {
            app.ports.receiveFromJs.send({
                type: 'gameError',
                error: result.error || 'Mouvement refusé'
            });
        }
    } catch (e) {
        console.error('makeMove error:', e);
        app.ports.receiveFromJs.send({
            type: 'gameError',
            error: e.message || 'Erreur réseau'
        });
    }
}

// ============================================================================
// HELPERS
// ============================================================================

function parseGameState(sessionState) {
    if (!sessionState) {
        return {
            sessionCode: '',
            state: 0,
            players: [],
            currentTurn: null
        };
    }

    return {
        sessionCode: sessionState.sessionId || '',
        state: sessionState.state || 0,
        players: (sessionState.players || []).map(p => ({
            id: p.id || '',
            name: p.name || 'Joueur',
            score: p.score || 0,
            isReady: p.isReady || false,
            isConnected: p.isConnected !== false
        })),
        currentTurn: sessionState.currentPlayerId || null
    };
}

function saveAuth(token, user) {
    localStorage.setItem(TOKEN_KEY, token);
    localStorage.setItem(USER_KEY, JSON.stringify(user));
}

function loadAuth() {
    const token = localStorage.getItem(TOKEN_KEY);
    const userStr = localStorage.getItem(USER_KEY);
    const user = userStr ? JSON.parse(userStr) : null;
    return { token, user };
}

function clearAuth() {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
}
