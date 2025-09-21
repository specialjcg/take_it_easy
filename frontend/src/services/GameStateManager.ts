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
        // ✅ SUPPRESSION DES LOGS POUR ÉVITER POLLUTION CONSOLE PENDANT POLLING
        // console.log('🔍 DEBUG convertSessionState - sessionState:', sessionState);
        // console.log('🔍 DEBUG convertSessionState - gameMode depuis proto:', sessionState.gameMode);

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
            currentTurn: sessionState.currentPlayerId,
            gameMode: sessionState.gameMode || 'multiplayer' // 🔥 AJOUT DU GAMEMMODE !
        };

        // console.log('🔍 DEBUG convertSessionState - gameState converti:', gameState);
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
                this.lastPlateauTilesHash = newPlateauHash;
                setPlateauTiles(newPlateauTiles);
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
                    this.lastAvailablePositionsHash = newPositionsHash;
                    setAvailablePositions(newPositions);
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
     * ✅ VERSION VIEWER AVEC LOGS RÉDUITS AUSSI
     */
    static updatePlateauTilesForViewer(
        gameState: any,
        setPlateauTiles: (tiles: {[playerId: string]: string[]}) => void,
        setAvailablePositions: (positions: number[]) => void,
        session: () => { playerId: string } | null,
    ) {
        console.log('👁️ VIEWER DEBUG: Fonction appelée', { gameState, session: session() });

        if (gameState.player_plateaus) {
            console.log('👁️ VIEWER DEBUG: player_plateaus trouvé', gameState.player_plateaus);

            const currentSession = session();
            if (currentSession && currentSession.playerId.includes('viewer')) {
                console.log('👁️ VIEWER DEBUG: Session viewer confirmée', currentSession.playerId);

                const mctsPlateau = gameState.player_plateaus?.['mcts_ai'];
                console.log('👁️ VIEWER DEBUG: Plateau MCTS', mctsPlateau);

                if (mctsPlateau) {
                    const newPlateauTiles = { 'mcts_ai': mctsPlateau.tile_images || [] };
                    console.log('👁️ VIEWER DEBUG: Nouveau plateau MCTS', newPlateauTiles);

                    // ✅ COMPARAISON POUR VIEWER AUSSI
                    const newPlateauHash = this.generateHash(newPlateauTiles);

                    if (newPlateauHash !== this.lastPlateauTilesHash) {
                        console.log('👀 VIEWER: plateau MCTS mis à jour!', newPlateauTiles);
                        this.lastPlateauTilesHash = newPlateauHash;
                        setPlateauTiles(newPlateauTiles);
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
     * 🚀 MISE À JOUR OPTIMISTE POUR RÉACTIVITÉ IMMÉDIATE
     */
    static updatePlateauTilesOptimistic(
        position: number,
        currentTile: string,
        plateauTiles: () => {[playerId: string]: string[]},
        setPlateauTiles: (tiles: {[playerId: string]: string[]}) => void,
        session: () => { playerId: string } | null,
        currentTileImage?: string
    ) {
        const currentSession = session();
        if (!currentSession) return;

        const currentPlateaus = plateauTiles();
        const playerPlateau = currentPlateaus[currentSession.playerId] || [];

        // Créer nouveau plateau avec la tuile placée optimistiquement
        const newPlayerPlateau = [...playerPlateau];
        
        // Utiliser l'image si fournie, sinon générer à partir du nom de tuile
        const tileImageToUse = currentTileImage || `../image/${currentTile.replace('-', '')}.png`;
        newPlayerPlateau[position] = tileImageToUse;

        const newPlateaus = {
            ...currentPlateaus,
            [currentSession.playerId]: newPlayerPlateau
        };

        setPlateauTiles(newPlateaus);
    }

    /**
     * ✅ FONCTION POUR RESET LE CACHE (quand on change de session)
     */
    static resetCache() {
        // console.log('🧹 RESET GAMESTATE CACHE'); // Log désactivé
        this.lastPlateauTilesHash = '';
        this.lastAvailablePositionsHash = '';
    }

    /**
     * Ouvrir une session MCTS dans une nouvelle fenêtre
     */
    static openMctsSession(session: () => { sessionCode: string } | null) {
        const currentSession = session();
        if (!currentSession) return;

        // Créer un viewer spécialisé pour voir le plateau MCTS
        const mctsUrl = `${window.location.origin}${window.location.pathname}?` +
            `sessionCode=${currentSession.sessionCode}&` +
            `playerId=mcts_viewer&` +
            `playerName=${encodeURIComponent('🔍 MCTS Viewer')}&` +
            `mode=mcts_view`;

        window.open(mctsUrl, '_blank', 'width=1200,height=800');
    }

    /**
     * Gestion de l'auto-connexion via paramètres URL et mode single-player
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

        // Mode viewer spécifique (viewer normal et mcts_view)
        if (sessionCode && playerId && playerName && (mode === 'viewer' || mode === 'mcts_view')) {
            setPlayerName(decodeURIComponent(playerName));
            setSessionCode(sessionCode);

            setTimeout(async () => {
                try {
                    await joinSession();
                    console.log(`🔍 ${mode === 'mcts_view' ? 'MCTS Viewer' : 'Viewer'} connecté à la session ${sessionCode}`);
                } catch (error) {
                    console.error(`❌ Erreur connexion ${mode}:`, error);
                }
            }, 1000);
            return;
        }

        // 🎮 AUTO-CONNEXION DÉSACTIVÉE - Utiliser le sélecteur de mode
        // La sélection de mode se fait maintenant via l'interface GameModeSelector
        if (!sessionCode && !mode) {
            console.log('🎮 Sélection de mode activée - pas d\'auto-connexion');
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