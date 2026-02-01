// components/ui/HexagonalGameBoard.tsx - VERSION SIMPLE ET STABLE
import {Component, createEffect, createMemo, createSignal, onCleanup, Show, untrack} from 'solid-js';


interface HexagonalGameBoardProps {
    plateauTiles: () => {[playerId: string]: string[]};
    availablePositions: () => number[];
    myTurn: () => boolean;
    session: () => { playerId: string } | null;
    onTileClick: (position: number) => void;
    currentTile?: () => string | null;
    isGameStarted?: () => boolean;
    // Pour afficher un plateau spécifique (mode lecture seule à la fin)
    displayPlayerId?: string;
    readOnly?: boolean;
    size?: 'normal' | 'small';
}

export const HexagonalGameBoard: Component<HexagonalGameBoardProps> = (props) => {
    let canvasRef: HTMLCanvasElement | undefined;



    // ✅ ÉTAT LOCAL OPTIMISÉ
    const [imageCache, setImageCache] = createSignal<Map<string, HTMLImageElement>>(new Map());
    const [isCanvasInitialized, setIsCanvasInitialized] = createSignal(false);
    const [lastRenderedHash, setLastRenderedHash] = createSignal('');

    // Positions hexagonales du plateau
    const hexPositions = [
        [-2, 2], [-2.3, 4], [-2.65, 6],
        [-1, 1], [-1.3, 3], [-1.6, 5], [-1.95, 7],
        [0, 0], [-0.3, 2], [-0.6, 4], [-0.9, 6], [-1.25, 8],
        [0.7, 1], [0.4, 3], [0.1, 5], [-0.2, 7],
        [1.4, 2], [1.1, 4], [0.8, 6]
    ];

    // Taille adaptative selon le mode
    const isSmall = props.size === 'small';
    const hexRadius = isSmall ? 28 : 42;
    const hexWidth = Math.sqrt(3) * hexRadius;
    const hexHeight = 2 * hexRadius;
    const offsetY = 0.45 * hexHeight;
    const canvasSize = isSmall ? 400 : 600;

    /**
     * 🚀 MEMO ULTRA-OPTIMISÉ POUR PERFORMANCE UX
     */
    type TilesData = { key: string, tiles: string[] };
    const stableTilesData = createMemo<TilesData | undefined>((prev?: TilesData) => {
        const allPlateaus = props.plateauTiles();

        // Si displayPlayerId est fourni, l'utiliser directement
        if (props.displayPlayerId) {
            const playerTiles = allPlateaus[props.displayPlayerId] || [];
            const realTiles = playerTiles.map((t, i) =>
                (t && t !== '' && !t.includes('000')) ? `${i}:${t.slice(-6)}` : ''
            ).filter(Boolean);
            const contentKey = `display-${props.displayPlayerId}-${playerTiles.length}-${realTiles.join('|')}`;
            const result: TilesData = { key: contentKey, tiles: playerTiles };
            return (prev && prev.key === contentKey) ? prev : result;
        }

        const currentSession = props.session();
        if (!currentSession) {
            const result: TilesData = { key: 'no-session', tiles: [] };
            return prev && prev.key === result.key ? prev : result;
        }

        const isViewerMode = currentSession.playerId.includes('viewer');
        const playerTiles = isViewerMode ?
            (allPlateaus['mcts_ai'] || []) :
            (allPlateaus[currentSession.playerId] || []);

        // ✅ HASH LÉGER - Seulement positions avec tuiles + longueur
        const realTiles = playerTiles.map((t, i) =>
            (t && t !== '' && !t.includes('000')) ? `${i}:${t.slice(-6)}` : ''
        ).filter(Boolean);

        const contentKey = `${currentSession.playerId}-${playerTiles.length}-${realTiles.join('|')}`;

        const result: TilesData = { key: contentKey, tiles: playerTiles };

        // ✅ RETURNER LA MÊME RÉFÉRENCE SI LE CONTENU EST IDENTIQUE
        return (prev && prev.key === contentKey) ? prev : result;
    });
    /**
     * 🚀 CACHE D'IMAGES ULTRA-RAPIDE
     */
    const loadImageCached = (src: string): Promise<HTMLImageElement> => {
        return new Promise((resolve) => {
            if (!src || src === '' || src.includes('000')) {
                resolve(new Image(1, 1));
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
            img.onerror = () => resolve(new Image(1, 1));
            img.src = src;
        });
    };

    /**
     * ✅ DESSINER UN HEXAGONE OPTIMISÉ
     */
    const drawHexagon = (ctx: CanvasRenderingContext2D, x: number, y: number, filled = true) => {
        const angleStep = Math.PI / 3;

        ctx.beginPath();
        for (let i = 0; i < 6; i++) {
            const angle = angleStep * i;
            const xOffset = x + hexRadius * Math.cos(angle);
            const yOffset = y + hexRadius * Math.sin(angle);
            if (i === 0) ctx.moveTo(xOffset, yOffset);
            else ctx.lineTo(xOffset, yOffset);
        }
        ctx.closePath();

        if (filled) {
            ctx.fillStyle = '#1a1a1a';
            ctx.fill();
        }
        ctx.strokeStyle = '#666666';
        ctx.lineWidth = 1;
        ctx.stroke();
    };


    /**
     * 🎯 DETECTION DE CLIC
     */
    const isPointInHexagon = (pointX: number, pointY: number, hexX: number, hexY: number, radius: number): boolean => {
        const dx = pointX - hexX;
        const dy = pointY - hexY;
        return Math.sqrt(dx * dx + dy * dy) < radius;
    };

    /**
     * 🚀 GESTION CLIC ULTRA-RAPIDE AVEC FEEDBACK VISUEL + DEBUG
     */
    const handleCanvasClick = (e: MouseEvent) => {
        // Ignorer les clics en mode lecture seule
        if (props.readOnly) return;

        const timestamp = performance.now();
        console.log(`🎯 [${timestamp.toFixed(0)}ms] CLIC DÉTECTÉ sur canvas`);

        const currentSession = untrack(() => props.session());
        const myTurn = props.myTurn();
        const isGameStarted = props.isGameStarted?.() ?? true;
        const currentTile = props.currentTile?.();

        console.log(`🔍 [${timestamp.toFixed(0)}ms] État clic:`, {
            hasSession: !!currentSession,
            playerId: currentSession?.playerId,
            isViewer: currentSession?.playerId.includes('viewer'),
            myTurn,
            isGameStarted,
            currentTile,
            availablePositions: props.availablePositions().length
        });

        if (!currentSession || currentSession.playerId.includes('viewer') || !myTurn) {
            console.log(`❌ [${timestamp.toFixed(0)}ms] CLIC BLOQUÉ - raison:`, {
                noSession: !currentSession,
                isViewer: currentSession?.playerId.includes('viewer'),
                notMyTurn: !myTurn
            });
            return;
        }

        if (!canvasRef) return;

        const rect = canvasRef.getBoundingClientRect();
        const clickX = e.clientX - rect.left;
        const clickY = e.clientY - rect.top;

        const gridOriginX = canvasRef.width / 2 - hexWidth;
        const gridOriginY = canvasRef.height / 2 - 2 * offsetY;

        console.log(`🎯 [${timestamp.toFixed(0)}ms] Recherche position pour clic (${clickX.toFixed(0)}, ${clickY.toFixed(0)})`);

        for (let index = 0; index < hexPositions.length; index++) {
            const [q, r] = hexPositions[index];
            const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
            const y = gridOriginY + r * offsetY - 50;

            if (isPointInHexagon(clickX, clickY, x, y, hexRadius)) {
                console.log(`🎯 [${timestamp.toFixed(0)}ms] Position détectée: ${index} (${x.toFixed(0)}, ${y.toFixed(0)})`);

                const availablePos = untrack(() => props.availablePositions());
                console.log(`🔍 [${timestamp.toFixed(0)}ms] Positions disponibles:`, availablePos);

                if (availablePos.includes(index)) {
                    console.log(`✅ [${timestamp.toFixed(0)}ms] Position ${index} autorisée - APPEL onTileClick`);

                    // 🚀 FEEDBACK VISUEL IMMÉDIAT
                    const ctx = canvasRef.getContext('2d');
                    if (ctx) {
                        ctx.save();
                        ctx.strokeStyle = '#4ade80';
                        ctx.lineWidth = 3;
                        drawHexagon(ctx, x, y, false);
                        ctx.restore();

                        // Reset après 150ms
                        setTimeout(() => {
                            if (ctx) drawHexagon(ctx, x, y, false);
                        }, 150);
                    }

                    const startOnTileClick = performance.now();
                    console.log(`🚀 [${startOnTileClick.toFixed(0)}ms] DÉBUT appel props.onTileClick(${index})`);

                    props.onTileClick(index);

                    const endOnTileClick = performance.now();
                    console.log(`⏱️ [${endOnTileClick.toFixed(0)}ms] FIN appel onTileClick - durée: ${(endOnTileClick - startOnTileClick).toFixed(1)}ms`);
                } else {
                    console.log(`❌ [${timestamp.toFixed(0)}ms] Position ${index} NON autorisée`);
                }
                return;
            }
        }

        console.log(`❌ [${timestamp.toFixed(0)}ms] Aucune position détectée pour ce clic`);
    };

    /**
     * 🎯 CREATEEFFECT AVEC CACHE GLOBAL PERSISTANT - RÉSOUT RE-MOUNTING
     */

    /**
     * 🚀 RENDU ULTRA-OPTIMISÉ - ÉVITE REDRAWS INUTILES
     */
    const renderCanvas = async (tiles: string[]) => {
        if (!canvasRef) return;
        
        const ctx = canvasRef.getContext('2d');
        if (!ctx) return;

        const gridOriginX = canvasRef.width / 2 - hexWidth;
        const gridOriginY = canvasRef.height / 2 - 2 * offsetY;

        // ✅ INITIALISATION SEULEMENT UNE FOIS
        if (!isCanvasInitialized()) {
            ctx.fillStyle = '#1e1e1e';
            ctx.fillRect(0, 0, canvasRef.width, canvasRef.height);
            
            // Dessiner tous les hexagones vides
            hexPositions.forEach(([q, r]) => {
                const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
                const y = gridOriginY + r * offsetY - 50;
                drawHexagon(ctx, x, y, true);
            });
            
            setIsCanvasInitialized(true);
        }

        // ✅ MISE À JOUR DIFFÉRENTIELLE DES TUILES
        const tilePromises = tiles.map(async (tile, index) => {
            if (!tile || tile === '' || tile.includes('000')) return;

            const [q, r] = hexPositions[index];
            const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
            const y = gridOriginY + r * offsetY - 50;

            try {
                const img = await loadImageCached(tile);
                if (img.width > 1) {
                    // Redessiner seulement cette zone
                    ctx.save();
                    ctx.beginPath();
                    for (let i = 0; i < 6; i++) {
                        const angle = (Math.PI / 3) * i;
                        const xOff = x + hexRadius * Math.cos(angle);
                        const yOff = y + hexRadius * Math.sin(angle);
                        if (i === 0) ctx.moveTo(xOff, yOff);
                        else ctx.lineTo(xOff, yOff);
                    }
                    ctx.closePath();
                    ctx.clip();

                    // Fond + image - échelle adaptée à la taille des hexagones
                    drawHexagon(ctx, x, y, true);
                    const scaleFactor = isSmall ? 3.0 : 2.0;
                    const scaledWidth = img.width / scaleFactor;
                    const scaledHeight = img.height / scaleFactor;
                    ctx.drawImage(img, x - scaledWidth/2, y - scaledHeight/2, scaledWidth, scaledHeight);
                    ctx.restore();
                    
                    // Contour par-dessus
                    drawHexagon(ctx, x, y, false);
                }
            } catch (e) {
                // Silent
            }
        });

        await Promise.all(tilePromises);
    };

    createEffect(() => {
        const tilesData = stableTilesData();
        if (!tilesData) return;
        const currentHash = tilesData.key;
        // ✅ ÉVITER REDRAWS SI RIEN N'A CHANGÉ
        if (currentHash === lastRenderedHash()) return;
        setLastRenderedHash(currentHash);
        renderCanvas(tilesData.tiles);
    });

    onCleanup(() => {
        setIsCanvasInitialized(false);
    });

    return (
        <div class="classic-board-area">
            <canvas
                ref={canvasRef!}
                width={canvasSize}
                height={canvasSize}
                class="classic-game-canvas"
                onClick={handleCanvasClick}
                style={{
                    cursor: (!props.readOnly && props.myTurn() && !props.session()?.playerId.includes('viewer')) ? 'pointer' : 'default',
                    'will-change': 'transform',
                    width: `${canvasSize}px`,
                    height: `${canvasSize}px`
                }}
            />
        </div>
    );
};