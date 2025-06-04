// services/GameStateManager.ts - Utilitaires et conversions d'état
import type {GameState as ProtoGameState, Player as ProtoPlayer} from '../generated/common';
import type {GameState, Player} from '../hooks/useGameState';

/**
 * Service pour la gestion et conversion des états de jeu
 * Centralise les utilitaires dispersés dans le composant principal
 */
export class GameStateManager {

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

    // ❌ FONCTIONS SUPPRIMÉES - Plus nécessaires car backend gère tout
    // static generateTileImagePath() - SUPPRIMÉ
    // static generateTileImagePathFromArray() - SUPPRIMÉ

    /**
     * Mettre à jour les plateaux de joueurs à partir de l'état de jeu
     */
    static updatePlateauTiles(
        gameState: any,
        setPlateauTiles: (tiles: { [playerId: string]: string[] }) => void,
        setAvailablePositions: (positions: number[]) => void,
        session: () => { playerId: string } | null,
        addDebugLog: (message: string) => void
    ) {
        if (gameState.player_plateaus) {
            const newPlateauTiles: { [playerId: string]: string[] } = {};

            Object.entries(gameState.player_plateaus).forEach(([playerId, plateau]: [string, any]) => {
                // ✅ UTILISER les images du backend
                newPlateauTiles[playerId] = plateau.tile_images || [];
                addDebugLog(`🎨 ${playerId}: ${(plateau.tile_images || []).length} images backend`);
            });

            setPlateauTiles(newPlateauTiles);
        }

        // Mettre à jour les positions disponibles pour le joueur actuel
        const currentSession = session();
        if (currentSession && gameState.waiting_for_players?.includes(currentSession.playerId)) {
            const myPlateau = gameState.player_plateaus?.[currentSession.playerId];
            if (myPlateau) {
                // ✅ UTILISER les positions du backend
                setAvailablePositions(myPlateau.available_positions || []);
                addDebugLog(`📍 Positions disponibles: ${(myPlateau.available_positions || []).length}`);
            }
        } else {
            setAvailablePositions([]);
        }
    }

    /**
     * Ouvrir une session MCTS dans une nouvelle fenêtre
     */
    static openMctsSession(session: () => { sessionCode: string } | null, addDebugLog: (message: string) => void) {
        const currentSession = session();
        if (!currentSession) return;

        // 🔧 NOUVEAU: Utiliser un nom différent pour le viewer
        const mctsUrl = `${window.location.origin}${window.location.pathname}?` +
            `sessionCode=${currentSession.sessionCode}&` +
            `playerId=mcts_viewer&` +                                    // ✅ ID différent
            `playerName=${encodeURIComponent('🔍 MCTS Viewer')}&` +       // ✅ Nom différent
            `mode=viewer`;

        window.open(mctsUrl, '_blank', 'width=1200,height=800');
        addDebugLog(`🔗 Session MCTS ouverte: ${mctsUrl}`);
    }

    /**
     * Gestion de l'auto-connexion via paramètres URL
     */
    static handleAutoConnection(
        setPlayerName: (name: string) => void,
        setSessionCode: (code: string) => void,
        addDebugLog: (message: string) => void,
        joinSession: () => Promise<void>
    ) {
        const urlParams = new URLSearchParams(window.location.search);
        const sessionCode = urlParams.get('sessionCode');
        const playerId = urlParams.get('playerId');
        const playerName = urlParams.get('playerName');
        const mode = urlParams.get('mode');

        if (sessionCode && playerId && playerName && mode === 'viewer') {
            // Auto-connexion pour la vue MCTS
            setPlayerName(decodeURIComponent(playerName));
            setSessionCode(sessionCode);

            addDebugLog(`🔗 Auto-connexion mode viewer: ${playerName} à ${sessionCode}`);

            setTimeout(async () => {
                try {
                    await joinSession();
                    addDebugLog(`✅ Connexion réussie en mode viewer`);
                } catch (error) {
                    addDebugLog(`❌ Erreur connexion viewer: ${error}`);
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
        addDebugLog: (message: string) => void
    ) {
        const tile = currentTile();
        const image = currentTileImage();

        if (tile && image) {
            const hash = `${tile}-${image}`;
            if (hash !== lastTileHash()) {
                setImageCache(image);
                setLastTileHash(hash);
                addDebugLog(`🔒 Image verrouillée: ${tile}`);
            }
        }
    }

// Dans GameStateManager.ts - NOUVELLE fonction pour viewer MCTS
    // Dans GameStateManager.ts - CORRIGER updatePlateauTilesForViewer
    static updatePlateauTilesForViewer(
        gameState: any,
        setPlateauTiles: (tiles: {[playerId: string]: string[]}) => void,
        setAvailablePositions: (positions: number[]) => void,
        session: () => { playerId: string } | null,
        addDebugLog: (message: string) => void
    ) {
        if (gameState.player_plateaus) {
            // 🔧 NOUVEAU: Pour le viewer, afficher SEULEMENT le plateau MCTS
            const currentSession = session();
            if (currentSession && currentSession.playerId.includes('viewer')) {
                const mctsPlateau = gameState.player_plateaus?.['mcts_ai'];
                if (mctsPlateau) {
                    // ✅ UTILISER les images du backend pour MCTS
                    setPlateauTiles({ 'mcts_ai': mctsPlateau.tile_images || [] });

                    // ✅ UTILISER les positions du backend pour MCTS
                    setAvailablePositions(mctsPlateau.available_positions || []);
                    addDebugLog(`👁️ VIEWER: Plateau MCTS uniquement - ${(mctsPlateau.tile_images || []).filter((t: string) => t !== '').length} tuiles placées`);
                    return; // ✅ SORTIR ICI pour éviter la logique normale
                }
            }

            // Logique normale pour les vrais joueurs
            const newPlateauTiles: {[playerId: string]: string[]} = {};
            Object.entries(gameState.player_plateaus).forEach(([playerId, plateau]: [string, any]) => {
                // ✅ UTILISER les images du backend
                newPlateauTiles[playerId] = plateau.tile_images || [];
            });
            setPlateauTiles(newPlateauTiles);

            // Reste de la logique normale...
        }
    }
}