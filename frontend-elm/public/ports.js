/**
 * ports.js - JavaScript interop for Elm
 * Uses the bundled gRPC-Web client for game communication
 * and direct fetch for Auth API
 */

// Configuration - Auto-detect environment
const IS_PRODUCTION = window.location.hostname !== 'localhost' && window.location.hostname !== '127.0.0.1';
const AUTH_API_BASE = IS_PRODUCTION
    ? '/auth'  // Production: nginx reverse proxy
    : 'http://localhost:51051/auth';  // Development: direct backend

// LocalStorage keys
const TOKEN_KEY = 'auth_token';
const USER_KEY = 'auth_user';

// Current player ID (set on session create/join)
let currentPlayerId = null;

// Cache player names from session events
let playerNames = {};

function getPlayerName(id) {
    if (id === 'mcts_ai') return 'IA';
    return playerNames[id] || 'Joueur';
}

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
                case 'forgotPassword':
                    await handleForgotPassword(app, message.email);
                    break;
                case 'resetPassword':
                    await handleResetPassword(app, message.token, message.newPassword);
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
                case 'pollSession':
                    await handlePollSession(app, message.sessionId);
                    break;
                case 'startTurn':
                    await handleStartTurn(app, message.sessionId, message.forcedTile);
                    break;
                case 'playMove':
                    await handlePlayMove(app, message.sessionId, message.playerId, message.position);
                    break;

                // ========== REAL GAME MODE (Jeu Réel) ==========
                case 'getAiMove':
                    await handleGetAiMove(app, message.tileCode, message.boardState, message.availablePositions, message.turnNumber);
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

async function handleForgotPassword(app, email) {
    try {
        const response = await fetch(`${AUTH_API_BASE}/forgot-password`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email })
        });

        const data = await response.json();

        if (response.ok) {
            app.ports.receiveFromJs.send({
                type: 'forgotPasswordSuccess',
                message: data.message || 'Si un compte existe avec cet email, un lien de réinitialisation a été envoyé.'
            });
        } else {
            app.ports.receiveFromJs.send({
                type: 'forgotPasswordFailure',
                error: data.error || 'Erreur lors de l\'envoi'
            });
        }
    } catch (error) {
        console.error('Forgot password error:', error);
        app.ports.receiveFromJs.send({
            type: 'forgotPasswordFailure',
            error: 'Erreur de connexion au serveur'
        });
    }
}

async function handleResetPassword(app, token, newPassword) {
    try {
        const response = await fetch(`${AUTH_API_BASE}/reset-password`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ token, new_password: newPassword })
        });

        const data = await response.json();

        if (response.ok) {
            app.ports.receiveFromJs.send({
                type: 'resetPasswordSuccess',
                message: data.message || 'Mot de passe réinitialisé avec succès !'
            });
        } else {
            app.ports.receiveFromJs.send({
                type: 'resetPasswordFailure',
                error: data.error || 'Lien invalide ou expiré'
            });
        }
    } catch (error) {
        console.error('Reset password error:', error);
        app.ports.receiveFromJs.send({
            type: 'resetPasswordFailure',
            error: 'Erreur de connexion au serveur'
        });
    }
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
            currentPlayerId = result.playerId;
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
            currentPlayerId = result.playerId;
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
    playerNames = {};
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
// SESSION POLLING
// ============================================================================

async function handlePollSession(app, sessionId) {
    console.log('🔄 Polling session:', sessionId);
    if (!window.grpcClient) {
        console.warn('🔄 Poll: no grpcClient');
        return;
    }

    try {
        const result = await window.grpcClient.getSessionState(sessionId);
        console.log('🔄 Poll result:', result);
        if (result.success) {
            const gameState = parseGameState(result.sessionState);
            console.log('🔄 Poll parsed gameState:', gameState);
            app.ports.receiveFromJs.send({
                type: 'sessionPolled',
                gameState: gameState
            });
        }
    } catch (e) {
        console.warn('Poll session error:', e);
    }
}

// ============================================================================
// GAMEPLAY HANDLERS (via gRPC client)
// ============================================================================

async function handleStartTurn(app, sessionId, forcedTile) {
    if (!window.grpcClient) {
        app.ports.receiveFromJs.send({
            type: 'gameError',
            error: 'gRPC client not loaded'
        });
        return;
    }

    try {
        const result = await window.grpcClient.startTurn(sessionId, forcedTile);
        console.log('startTurn result:', result);

        if (result.success) {
            let positions = [];
            let players = [];

            if (result.gameState) {
                try {
                    const gs = typeof result.gameState === 'string'
                        ? JSON.parse(result.gameState)
                        : result.gameState;

                    // Extraire les positions disponibles du joueur courant
                    if (gs.player_plateaus && currentPlayerId && gs.player_plateaus[currentPlayerId]) {
                        positions = gs.player_plateaus[currentPlayerId].available_positions || [];
                    } else if (gs.player_plateaus) {
                        // Fallback: premier joueur non-IA
                        Object.entries(gs.player_plateaus).forEach(([id, plateau]) => {
                            if (plateau.available_positions && id !== 'mcts_ai' && positions.length === 0) {
                                positions = plateau.available_positions;
                            }
                        });
                    }

                    // Extraire les joueurs avec leurs scores
                    if (gs.scores) {
                        Object.entries(gs.scores).forEach(([id, score]) => {
                            players.push({
                                id: id,
                                name: getPlayerName(id),
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
                players: players,
                waitingForPlayers: result.waitingForPlayers || []
            });
        } else {
            // Check if game is finished - fetch final state instead of showing error
            await handleStartTurnGameOver(app, sessionId, result.error);
        }
    } catch (e) {
        console.error('startTurn error:', e);
        await handleStartTurnGameOver(app, sessionId, e.message);
    }
}

async function handleStartTurnGameOver(app, sessionId, errorMsg) {
    try {
        const stateResult = await window.grpcClient.getGameState(sessionId);
        if (stateResult.success && stateResult.isGameFinished) {
            sendGameFinishedFromState(app, stateResult.gameState);
            return;
        }
    } catch (e) {
        // ignore - fall through to error
    }
    // Not a game-finished case - send as regular error
    app.ports.receiveFromJs.send({
        type: 'gameError',
        error: errorMsg || 'Erreur start turn'
    });
}

function sendGameFinishedFromState(app, rawGameState) {
    let boardData = rawGameState;
    if (typeof boardData === 'string') {
        try { boardData = JSON.parse(boardData); } catch (e) { boardData = {}; }
    }
    if (boardData?.boardState) {
        try {
            boardData = typeof boardData.boardState === 'string'
                ? JSON.parse(boardData.boardState) : boardData.boardState;
        } catch (e) { /* keep boardData */ }
    }

    const players = [];
    const plateaus = {};
    const scores = boardData?.scores || {};

    if (boardData && boardData.player_plateaus) {
        Object.entries(boardData.player_plateaus).forEach(([id, plateau]) => {
            players.push({
                id: id,
                name: getPlayerName(id),
                score: scores[id] || 0,
                isReady: true,
                isConnected: true
            });
            const tileImages = [];
            if (plateau.tiles) {
                for (let i = 0; i < 19; i++) {
                    const tile = plateau.tiles[i];
                    if (tile && (tile[0] !== 0 || tile[1] !== 0 || tile[2] !== 0)) {
                        tileImages.push(`image/${tile[0]}${tile[1]}${tile[2]}.png`);
                    } else {
                        tileImages.push('');
                    }
                }
            }
            plateaus[id] = tileImages;
        });
    }

    console.log('🏁 Game finished detected from startTurn:', { players, plateaus });
    app.ports.receiveFromJs.send({
        type: 'gameFinished',
        players: players,
        plateaus: plateaus
    });
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
            // Parse game state to extract AI data
            let gameState = result.newGameState;
            if (typeof gameState === 'string') {
                try {
                    gameState = JSON.parse(gameState);
                } catch (e) {
                    gameState = {};
                }
            }

            let boardData = gameState;
            if (gameState?.boardState) {
                try {
                    boardData = typeof gameState.boardState === 'string'
                        ? JSON.parse(gameState.boardState)
                        : gameState.boardState;
                } catch (e) {
                    boardData = {};
                }
            }

            // Extract AI tiles and score for Solo mode display
            let aiTiles = [];
            let aiScore = 0;
            const scores = boardData?.scores || {};

            if (boardData && boardData.player_plateaus && boardData.player_plateaus.mcts_ai) {
                const aiPlateau = boardData.player_plateaus.mcts_ai;
                aiScore = scores.mcts_ai || 0;

                if (aiPlateau.tiles) {
                    for (let i = 0; i < 19; i++) {
                        const tile = aiPlateau.tiles[i];
                        if (tile && (tile[0] !== 0 || tile[1] !== 0 || tile[2] !== 0)) {
                            aiTiles.push(`image/${tile[0]}${tile[1]}${tile[2]}.png`);
                        } else {
                            aiTiles.push('');
                        }
                    }
                }
            }

            app.ports.receiveFromJs.send({
                type: 'movePlayed',
                position: position,
                points: result.pointsEarned || 0,
                aiTiles: aiTiles,
                aiScore: aiScore,
                isGameOver: result.isGameOver || false
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
                            name: getPlayerName(id),
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
// REAL GAME MODE - AI MOVE
// ============================================================================

async function handleGetAiMove(app, tileCode, boardState, availablePositions, turnNumber) {
    if (!window.grpcClient) {
        app.ports.receiveFromJs.send({
            type: 'aiMoveResult',
            success: false,
            position: -1,
            error: 'gRPC client not loaded'
        });
        return;
    }

    try {
        console.log('🤖 Calling getAiMove:', { tileCode, boardState, availablePositions, turnNumber });
        const result = await window.grpcClient.getAiMove(tileCode, boardState, availablePositions, turnNumber);
        console.log('🤖 getAiMove result:', result);

        if (result.success) {
            const msg = {
                type: 'aiMoveResult',
                position: result.recommendedPosition,
                error: ''
            };
            console.log('🤖 Sending to Elm:', msg);
            app.ports.receiveFromJs.send(msg);
        } else {
            // Fallback: position aléatoire parmi les disponibles
            const fallbackPosition = availablePositions.length > 0
                ? availablePositions[Math.floor(Math.random() * availablePositions.length)]
                : 0;
            console.warn('AI move failed, using fallback:', result.error);
            const fallbackMsg = {
                type: 'aiMoveResult',
                position: fallbackPosition,
                error: 'Fallback: ' + (result.error || 'AI non disponible')
            };
            console.log('🤖 Sending fallback to Elm:', fallbackMsg);
            app.ports.receiveFromJs.send(fallbackMsg);
        }
    } catch (e) {
        console.error('getAiMove error:', e);
        // Fallback aléatoire en cas d'erreur
        const fallbackPosition = availablePositions.length > 0
            ? availablePositions[Math.floor(Math.random() * availablePositions.length)]
            : 0;
        const errorMsg = {
            type: 'aiMoveResult',
            position: fallbackPosition,
            error: 'Fallback: erreur réseau'
        };
        console.log('🤖 Sending error fallback to Elm:', errorMsg);
        app.ports.receiveFromJs.send(errorMsg);
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

    const players = (sessionState.players || []).map(p => ({
        id: p.id || '',
        name: p.name || 'Joueur',
        score: p.score || 0,
        isReady: p.isReady || false,
        isConnected: p.isConnected !== false
    }));

    // Cache player names for use in gameplay events
    players.forEach(p => {
        if (p.id && p.name) {
            playerNames[p.id] = p.name;
        }
    });

    return {
        sessionCode: sessionState.sessionId || '',
        state: sessionState.state || 0,
        players: players,
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
