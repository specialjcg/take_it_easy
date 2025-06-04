// services/GameStateManager.ts - VERSION CORRIGÉE AVEC COMPARAISON
import type {GameState as ProtoGameState, Player as ProtoPlayer} from '../generated/common';
import type {GameState, Player} from '../hooks/useGameState';

/**
 * Service pour la gestion et conversion des états de jeu
 * Centralise les utilitaires dispersés dans le composant principal
 */
export class GameStateManager {

    // ✅ CACHE POUR ÉVITER LES MISES À JOUR INUTILES
    private static lastPlateauTilesHash = '';
    private static lastAvailablePositionsHash = '';

    /**
     * Convertir l'état de session reçu du backend vers le format local
     */
    static convertSessionState(sessionState: ProtoGameState): GameState {
        const gameState = {
            sessionCode: sessionState.sessionId || '',
            state: sessionState.state,
            players: sessionState.players.map((p: ProtoPlayer) => ({
                id: p.id,
                name: p.name,
                score: p.score,
                isReady: p.isReady,
                isConnected: p.isConnected,
                joinedAt: p.joinedAt.toString()
            })),
            boardState: sessionState.boardState || '{}',
            currentTurn: sessionState.currentPlayerId
        };

        return gameState;
    }

    /**
     * 🚀 FONCTION UTILITAIRE POUR GÉNÉRER UN HASH
     */
    private static generateHash(data: any): string {
        try {
            return JSON.stringify(data);
        } catch (e) {
            return String(data);
        }
    }

    /**
     * ✅ METTRE À JOUR LES PLATEAUX AVEC COMPARAISON
     */
    static updatePlateauTiles(
        gameState: any,
        setPlateauTiles: (tiles: { [playerId: string]: string[] }) => void,
        setAvailablePositions: (positions: number[]) => void,
        session: () => { playerId: string } | null,
    ) {
        if (gameState.player_plateaus) {
            const newPlateauTiles: { [playerId: string]: string[] } = {};

            Object.entries(gameState.player_plateaus).forEach(([playerId, plateau]: [string, any]) => {
                newPlateauTiles[playerId] = plateau.tile_images || [];
            });

            // ✅ COMPARAISON AVANT MISE À JOUR
            const newPlateauHash = this.generateHash(newPlateauTiles);

            if (newPlateauHash !== this.lastPlateauTilesHash) {
                console.log('🔄 PLATEAU TILES CHANGED', {
                    oldHash: this.lastPlateauTilesHash.slice(-20),
                    newHash: newPlateauHash.slice(-20)
                });

                this.lastPlateauTilesHash = newPlateauHash;
                setPlateauTiles(newPlateauTiles);
            } else {
                console.log('⏩ PLATEAU TILES UNCHANGED - SKIP UPDATE');
            }
        }

        // ✅ MÊME LOGIQUE POUR LES POSITIONS DISPONIBLES
        const currentSession = session();
        if (currentSession && gameState.waiting_for_players?.includes(currentSession.playerId)) {
            const myPlateau = gameState.player_plateaus?.[currentSession.playerId];
            if (myPlateau) {
                const newPositions = myPlateau.available_positions || [];
                const newPositionsHash = this.generateHash(newPositions);

                if (newPositionsHash !== this.lastAvailablePositionsHash) {
                    console.log('🔄 AVAILABLE POSITIONS CHANGED');
                    this.lastAvailablePositionsHash = newPositionsHash;
                    setAvailablePositions(newPositions);
                } else {
                    console.log('⏩ AVAILABLE POSITIONS UNCHANGED - SKIP UPDATE');
                }
            }
        } else {
            // Reset positions si plus mon tour
            const emptyPositionsHash = this.generateHash([]);
            if (emptyPositionsHash !== this.lastAvailablePositionsHash) {
                this.lastAvailablePositionsHash = emptyPositionsHash;
                setAvailablePositions([]);
            }
        }
    }

    /**
     * ✅ VERSION VIEWER AVEC COMPARAISON AUSSI
     */
    static updatePlateauTilesForViewer(
        gameState: any,
        setPlateauTiles: (tiles: {[playerId: string]: string[]}) => void,
        setAvailablePositions: (positions: number[]) => void,
        session: () => { playerId: string } | null,
    ) {
        if (gameState.player_plateaus) {
            const currentSession = session();
            if (currentSession && currentSession.playerId.includes('viewer')) {
                const mctsPlateau = gameState.player_plateaus?.['mcts_ai'];
                if (mctsPlateau) {
                    const newPlateauTiles = { 'mcts_ai': mctsPlateau.tile_images || [] };

                    // ✅ COMPARAISON POUR VIEWER AUSSI
                    const newPlateauHash = this.generateHash(newPlateauTiles);

                    if (newPlateauHash !== this.lastPlateauTilesHash) {
                        console.log('🔄 VIEWER PLATEAU CHANGED');
                        this.lastPlateauTilesHash = newPlateauHash;
                        setPlateauTiles(newPlateauTiles);
                    } else {
                        console.log('⏩ VIEWER PLATEAU UNCHANGED - SKIP UPDATE');
                    }

                    // Positions pour viewer
                    const newPositions = mctsPlateau.available_positions || [];
                    const newPositionsHash = this.generateHash(newPositions);

                    if (newPositionsHash !== this.lastAvailablePositionsHash) {
                        this.lastAvailablePositionsHash = newPositionsHash;
                        setAvailablePositions(newPositions);
                    }

                    return;
                }
            }

            // ✅ FALLBACK - Logique normale avec comparaison
            this.updatePlateauTiles(gameState, setPlateauTiles, setAvailablePositions, session);
        }
    }

    /**
     * ✅ FONCTION POUR RESET LE CACHE (quand on change de session)
     */
    static resetCache() {
        console.log('🧹 RESET GAMESTATE CACHE');
        this.lastPlateauTilesHash = '';
        this.lastAvailablePositionsHash = '';
    }

    /**
     * Ouvrir une session MCTS dans une nouvelle fenêtre
     */
    static openMctsSession(session: () => { sessionCode: string } | null) {
        const currentSession = session();
        if (!currentSession) return;

        const mctsUrl = `${window.location.origin}${window.location.pathname}?` +
            `sessionCode=${currentSession.sessionCode}&` +
            `playerId=mcts_viewer&` +
            `playerName=${encodeURIComponent('🔍 MCTS Viewer')}&` +
            `mode=viewer`;

        window.open(mctsUrl, '_blank', 'width=1200,height=800');
    }

    /**
     * Gestion de l'auto-connexion via paramètres URL
     */
    static handleAutoConnection(
        setPlayerName: (name: string) => void,
        setSessionCode: (code: string) => void,
        joinSession: () => Promise<void>
    ) {
        const urlParams = new URLSearchParams(window.location.search);
        const sessionCode = urlParams.get('sessionCode');
        const playerId = urlParams.get('playerId');
        const playerName = urlParams.get('playerName');
        const mode = urlParams.get('mode');

        if (sessionCode && playerId && playerName && mode === 'viewer') {
            setPlayerName(decodeURIComponent(playerName));
            setSessionCode(sessionCode);

            setTimeout(async () => {
                try {
                    await joinSession();
                } catch (error) {
                    // Silent
                }
            }, 1000);
        }
    }

    /**
     * Gestion du cache d'images avec verrouillage
     */
    static updateImageCache(
        currentTile: () => string | null,
        currentTileImage: () => string | null,
        lastTileHash: () => string,
        setImageCache: (cache: string | null) => void,
        setLastTileHash: (hash: string) => void,
    ) {
        const tile = currentTile();
        const image = currentTileImage();

        if (tile && image) {
            const hash = `${tile}-${image}`;
            if (hash !== lastTileHash()) {
                setImageCache(image);
                setLastTileHash(hash);
            }
        }
    }
}