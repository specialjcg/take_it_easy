// components/ui/HexagonalGameBoard.tsx - VERSION SIMPLE ET STABLE
import {Component, createEffect, createMemo, createSignal, onCleanup, Show, untrack} from 'solid-js';

// ✅ CACHE GLOBAL PERSISTANT pour survivre aux re-créations de composant
const GLOBAL_BOARD_CACHE = new Map<string, {
    lastContentKey: string;
    backgroundDrawn: boolean;
    lastDrawnTiles: string[];
}>();

interface HexagonalGameBoardProps {
    plateauTiles: () => {[playerId: string]: string[]};
    availablePositions: () => number[];
    myTurn: () => boolean;
    session: () => { playerId: string } | null;
    onTileClick: (position: number) => void;
    currentTile?: () => string | null;
    isGameStarted?: () => boolean;
}

export const HexagonalGameBoard: Component<HexagonalGameBoardProps> = (props) => {
    let canvasRef: HTMLCanvasElement | undefined;

    // ✅ CLÉ UNIQUE POUR CE BOARD (basée sur la session)
    const getBoardKey = () => {
        const session = props.session();
        return session ? `board-${session.playerId}` : 'board-no-session';
    };

    // ✅ RÉCUPÉRER OU CRÉER LE CACHE PERSISTANT
    const getOrCreateCache = () => {
        const key = getBoardKey();
        if (!GLOBAL_BOARD_CACHE.has(key)) {
            GLOBAL_BOARD_CACHE.set(key, {
                lastContentKey: '',
                backgroundDrawn: false,
                lastDrawnTiles: []
            });
        }
        return GLOBAL_BOARD_CACHE.get(key)!;
    };

    // ✅ ÉTAT LOCAL AVEC CACHE D'IMAGES
    const [imageCache, setImageCache] = createSignal<Map<string, HTMLImageElement>>(new Map());

    // Positions hexagonales du plateau
    const hexPositions = [
        [-2, 2], [-2.3, 4], [-2.65, 6],
        [-1, 1], [-1.3, 3], [-1.6, 5], [-1.95, 7],
        [0, 0], [-0.3, 2], [-0.6, 4], [-0.9, 6], [-1.25, 8],
        [0.7, 1], [0.4, 3], [0.1, 5], [-0.2, 7],
        [1.4, 2], [1.1, 4], [0.8, 6]
    ];

    const hexRadius = 35;
    const hexWidth = Math.sqrt(3) * hexRadius;
    const hexHeight = 2 * hexRadius;
    const offsetY = 0.45 * hexHeight;

    /**
     * 🎯 MEMO ULTRA-STABLE AVEC COMPARAISON PROFONDE
     */
    const stableTilesData = createMemo((prev) => {
        const currentSession = props.session();
        if (!currentSession) {
            const result = { key: 'no-session', tiles: [], debugInfo: 'no-session' };
            return prev && prev.key === result.key ? prev : result;
        }

        const isViewerMode = currentSession.playerId.includes('viewer');
        const allPlateaus = props.plateauTiles();

        // 🔍 DEBUG: Voir si allPlateaus change de référence
        const plateauStringified = JSON.stringify(allPlateaus);
        
        let playerTiles: string[] = [];
        if (isViewerMode) {
            playerTiles = allPlateaus['mcts_ai'] || [];
        } else {
            playerTiles = allPlateaus[currentSession.playerId] || [];
        }

        // ✅ CLÉ ULTRA-STABLE: Hash du contenu réel ET structure
        const realTiles = playerTiles.filter(t => t && t !== '' && !t.includes('000'));
        const contentKey = `${currentSession.playerId}-${realTiles.length}-${plateauStringified.length}-${realTiles.join('|')}`;

        // 🔍 DEBUG: Traquer les changements
        const debugInfo = {
            playerId: currentSession.playerId,
            tilesCount: playerTiles.length,
            realTilesCount: realTiles.length,
            plateauKeys: Object.keys(allPlateaus),
            plateauSizes: Object.fromEntries(Object.entries(allPlateaus).map(([k,v]) => [k, v?.length || 0])),
            plateauStringifiedLength: plateauStringified.length,
            prevKey: prev?.key || 'none',
            timestamp: Date.now()
        };

        const result = {
            key: contentKey,
            tiles: playerTiles,
            realTiles: realTiles,
            debugInfo
        };

        // ✅ RETURNER LA MÊME RÉFÉRENCE SI LE CONTENU EST IDENTIQUE
        if (prev && prev.key === contentKey) {
            return prev;
        }
        
        return result;
    });
    /**
     * 🚀 CACHE D'IMAGES SIMPLE
     */
    const loadImageCached = (src: string): Promise<HTMLImageElement> => {
        return new Promise((resolve) => {
            if (!src || src === '' || src.includes('000')) {
                const emptyImg = new Image();
                emptyImg.width = 1;
                emptyImg.height = 1;
                resolve(emptyImg);
                return;
            }

            const cache = imageCache();
            if (cache.has(src)) {
                resolve(cache.get(src)!);
                return;
            }

            const img = new Image();
            img.onload = () => {
                const newCache = new Map(cache);
                newCache.set(src, img);
                setImageCache(newCache);
                resolve(img);
            };
            img.onerror = () => {
                const emptyImg = new Image();
                emptyImg.width = 1;
                emptyImg.height = 1;
                resolve(emptyImg);
            };
            img.src = src;
        });
    };

    /**
     * ✅ DESSINER UN HEXAGONE NEUTRE
     */
    const drawNeutralHexagon = (ctx: CanvasRenderingContext2D, x: number, y: number, radius: number) => {
        const angleStep = Math.PI / 3;

        ctx.beginPath();
        for (let i = 0; i < 6; i++) {
            const angle = angleStep * i;
            const xOffset = x + radius * Math.cos(angle);
            const yOffset = y + radius * Math.sin(angle);
            if (i === 0) ctx.moveTo(xOffset, yOffset);
            else ctx.lineTo(xOffset, yOffset);
        }
        ctx.closePath();

        ctx.fillStyle = '#1a1a1a';
        ctx.fill();
        ctx.strokeStyle = '#666666';
        ctx.lineWidth = 1;
        ctx.stroke();
    };

    /**
     * 🚀 FONCTION DRAW DIFFÉRENTIELLE - SEULEMENT LES CHANGEMENTS
     */
    const drawBackground = (ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement) => {
        console.log('🎨 DRAWING BACKGROUND - FORCE REDRAW EVERY TIME');
        
        // ✅ FORCER LE DESSIN À CHAQUE FOIS POUR ÉVITER L'ÉCRAN NOIR
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        ctx.fillStyle = '#1e1e1e';
        ctx.fillRect(0, 0, canvas.width, canvas.height);
        
        console.log('🎨 DRAWING BACKGROUND - Canvas cleared and filled');

        // Calculer l'origine
        const gridOriginX = canvas.width / 2 - hexWidth;
        const gridOriginY = canvas.height / 2 - 2 * offsetY;

        console.log('🎨 DRAWING BACKGROUND - Drawing hexagons', {
            gridOriginX,
            gridOriginY,
            hexPositionsCount: hexPositions.length
        });

        // Dessiner TOUS les hexagones neutres À CHAQUE FOIS
        hexPositions.forEach(([q, r], index) => {
            const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
            const y = gridOriginY + r * offsetY - 50;
            drawNeutralHexagon(ctx, x, y, hexRadius);
        });

        console.log('🎨 DRAWING BACKGROUND - All hexagons redrawn');
    };

    const drawSingleTile = async (ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement, index: number, tileImage: string) => {
        console.log(`🎯 DRAW SINGLE TILE ${index}:`, { tileImage: tileImage?.slice(0, 50) || 'empty' });
        
        if (!tileImage || tileImage === '' || tileImage.includes('000')) {
            console.log(`🎯 TILE ${index} SKIPPED - Empty tile`);
            return;
        }
        
        console.log(`🎯 TILE ${index} DRAWING - Valid tile`);
        
        // Le reste du code continue...

        // Calculer position
        const gridOriginX = canvas.width / 2 - hexWidth;
        const gridOriginY = canvas.height / 2 - 2 * offsetY;
        const [q, r] = hexPositions[index];
        const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
        const y = gridOriginY + r * offsetY - 50;

        try {
            const img = await loadImageCached(tileImage);
            const scaledWidth = img.width / 2.4;
            const scaledHeight = img.height / 2.4;

            // ✅ EFFACER SEULEMENT LA ZONE DE CET HEXAGONE
            ctx.save();
            ctx.beginPath();
            const angleStep = Math.PI / 3;
            for (let i = 0; i < 6; i++) {
                const angle = angleStep * i;
                const xOffset = x + hexRadius * Math.cos(angle);
                const yOffset = y + hexRadius * Math.sin(angle);
                if (i === 0) ctx.moveTo(xOffset, yOffset);
                else ctx.lineTo(xOffset, yOffset);
            }
            ctx.closePath();
            ctx.clip();

            // Redessiner le fond hexagonal
            drawNeutralHexagon(ctx, x, y, hexRadius);

            // Dessiner l'image
            ctx.drawImage(
                img,
                x - scaledWidth / 2,
                y - scaledHeight / 2,
                scaledWidth,
                scaledHeight
            );

            ctx.restore();

            // Redessiner le contour par-dessus
            ctx.beginPath();
            for (let i = 0; i < 6; i++) {
                const angle = angleStep * i;
                const xOffset = x + hexRadius * Math.cos(angle);
                const yOffset = y + hexRadius * Math.sin(angle);
                if (i === 0) ctx.moveTo(xOffset, yOffset);
                else ctx.lineTo(xOffset, yOffset);
            }
            ctx.closePath();
            ctx.strokeStyle = '#666666';
            ctx.lineWidth = 1;
            ctx.stroke();
        } catch (e) {
            // Silencieux
        }
    };

    /**
     * 🎯 DESSIN DIFFÉRENTIEL - SEULEMENT LES TUILES QUI ONT CHANGÉ
     */
    const drawHexagonalGridDifferential = async (ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement, tiles: string[]) => {
        const cache = getOrCreateCache();
        
        console.log('🎯 DIFFERENTIAL DRAW - Starting', {
            tilesLength: tiles.length,
            cacheLastDrawnTilesLength: cache.lastDrawnTiles.length
        });
        
        // 1. Dessiner le fond (une seule fois)
        drawBackground(ctx, canvas);

        // 2. Identifier les changements
        const changedIndices: number[] = [];
        for (let i = 0; i < tiles.length; i++) {
            if (tiles[i] !== cache.lastDrawnTiles[i]) {
                changedIndices.push(i);
            }
        }

        console.log('🎯 DIFFERENTIAL DRAW - Changes detected', {
            changedIndices,
            changedCount: changedIndices.length
        });

        // 3. Redessiner SEULEMENT les tuiles qui ont changé
        if (changedIndices.length > 0) {
            console.log('🎯 DIFFERENTIAL DRAW - Drawing changed tiles');
            const drawPromises = changedIndices.map(index => 
                drawSingleTile(ctx, canvas, index, tiles[index])
            );
            await Promise.all(drawPromises);
            console.log('🎯 DIFFERENTIAL DRAW - All changed tiles drawn');
        }

        // 4. Mettre à jour le cache des tuiles
        cache.lastDrawnTiles = [...tiles];
        console.log('🎯 DIFFERENTIAL DRAW - Completed');
    };

    /**
     * 🎯 DETECTION DE CLIC
     */
    const isPointInHexagon = (pointX: number, pointY: number, hexX: number, hexY: number, radius: number): boolean => {
        const dx = pointX - hexX;
        const dy = pointY - hexY;
        return Math.sqrt(dx * dx + dy * dy) < radius;
    };

    const handleCanvasClick = (e: MouseEvent) => {
        const currentSession = untrack(() => props.session());
        const isViewerMode = currentSession && currentSession.playerId.includes('viewer');

        if (isViewerMode || !props.myTurn()) {
            return;
        }

        if (!canvasRef) return;

        const rect = canvasRef.getBoundingClientRect();
        const clickX = e.clientX - rect.left;
        const clickY = e.clientY - rect.top;

        const gridOriginX = canvasRef.width / 2 - hexWidth;
        const gridOriginY = canvasRef.height / 2 - 2 * offsetY;

        for (let index = 0; index < hexPositions.length; index++) {
            const [q, r] = hexPositions[index];
            const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
            const y = gridOriginY + r * offsetY - 50;

            if (isPointInHexagon(clickX, clickY, x, y, hexRadius)) {
                const availablePos = untrack(() => props.availablePositions());
                if (availablePos.includes(index)) {
                    props.onTileClick(index);
                }
                return;
            }
        }
    };

    /**
     * 🎯 CREATEEFFECT AVEC CACHE GLOBAL PERSISTANT - RÉSOUT RE-MOUNTING
     */
    // ✅ VARIABLES PERSISTANTES HORS DU CREATEEFFECT
    let isDrawing = false;
    let redrawTimeout: ReturnType<typeof setTimeout> | undefined;

    createEffect(() => {
        const tilesData = stableTilesData();
        
        if (!canvasRef) return;
        
        const tiles = (tilesData as any)?.tiles || [];
        
        // ✅ SIMPLE: Redessiner à chaque changement de données
        const ctx = canvasRef.getContext('2d');
        if (ctx) {
            // Effacer le canvas
            ctx.clearRect(0, 0, canvasRef.width, canvasRef.height);
            
            // Dessiner le fond
            ctx.fillStyle = '#1e1e1e';
            ctx.fillRect(0, 0, canvasRef.width, canvasRef.height);

            // Calculer l'origine
            const gridOriginX = canvasRef.width / 2 - hexWidth;
            const gridOriginY = canvasRef.height / 2 - 2 * offsetY;

            // Dessiner TOUS les hexagones
            hexPositions.forEach(([q, r], index) => {
                const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
                const y = gridOriginY + r * offsetY - 50;
                
                // Dessiner l'hexagone neutre
                drawNeutralHexagon(ctx, x, y, hexRadius);
                
                // Si il y a une tuile pour cette position, la dessiner
                if (tiles[index] && tiles[index] !== '' && !tiles[index].includes('000')) {
                    // Dessiner la tuile par-dessus (version simplifiée)
                    loadImageCached(tiles[index]).then(img => {
                        const scaledWidth = img.width / 2.4;
                        const scaledHeight = img.height / 2.4;
                        ctx.drawImage(
                            img,
                            x - scaledWidth / 2,
                            y - scaledHeight / 2,
                            scaledWidth,
                            scaledHeight
                        );
                    });
                }
            });
        }
    });

    onCleanup(() => {
        if (redrawTimeout) {
            clearTimeout(redrawTimeout);
        }
    });

    return (
        <div class="classic-board-area">
            <canvas
                ref={canvasRef!}
                width="500"
                height="500"
                class="classic-game-canvas"
                onClick={handleCanvasClick}
                style={{
                    border: '2px solid #333',
                    'border-radius': '8px',
                    cursor: (props.myTurn() && !props.session()?.playerId.includes('viewer')) ? 'pointer' : 'default'
                }}
            />

            <div class="classic-instructions">
                <Show when={props.session()?.playerId.includes('viewer')}>
                    <p style={{ color: '#8b5cf6', 'font-weight': 'bold' }}>
                        👁️ Mode Observateur - Plateau MCTS affiché
                    </p>
                </Show>

                <Show when={!props.session()?.playerId.includes('viewer')}>
                    <Show when={props.myTurn() && props.availablePositions().length > 0}>
                        <p style={{ color: '#999', 'font-weight': 'bold' }}>
                            🎯 À votre tour - Cliquez sur un hexagone pour placer votre tuile
                        </p>
                    </Show>
                    <Show when={!props.myTurn()}>
                        <p style={{ color: '#666', 'font-style': 'italic' }}>
                            ⏳ En attente de votre tour
                        </p>
                    </Show>
                </Show>
            </div>
        </div>
    );
};