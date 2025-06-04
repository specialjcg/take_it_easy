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
}

export const HexagonalGameBoard: Component<HexagonalGameBoardProps> = (props) => {
    let canvasRef: HTMLCanvasElement | undefined;

    // ✅ ÉTAT MINIMAL - SEULEMENT L'ESSENTIEL
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
     * 🎯 MEMO STABLE QUI FILTRE LES VRAIS CHANGEMENTS
     */
    const stableTilesData = createMemo(() => {
        const currentSession = props.session();
        if (!currentSession) return { key: 'no-session', tiles: [] };

        const isViewerMode = currentSession.playerId.includes('viewer');
        const allPlateaus = props.plateauTiles();

        let playerTiles: string[] = [];
        if (isViewerMode) {
            playerTiles = allPlateaus['mcts_ai'] || [];
        } else {
            playerTiles = allPlateaus[currentSession.playerId] || [];
        }

        // ✅ CLÉ UNIQUE BASÉE SUR LE CONTENU RÉEL
        const realTiles = playerTiles.filter(t => t && t !== '' && !t.includes('000'));
        const contentKey = `${currentSession.playerId}-${realTiles.length}-${realTiles.join('|')}`;

        return {
            key: contentKey,
            tiles: playerTiles,
            realTiles: realTiles
        };
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
     * 🎯 FONCTION DRAW SIMPLE - PAS DE COMPARAISON COMPLIQUÉE
     */
    const drawHexagonalGrid = async (ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement, tiles: string[]) => {
        // Effacer le canvas
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        ctx.fillStyle = '#1e1e1e';
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        // Calculer l'origine
        const gridOriginX = canvas.width / 2 - hexWidth;
        const gridOriginY = canvas.height / 2 - 2 * offsetY;

        // Dessiner les hexagones neutres
        hexPositions.forEach(([q, r], index) => {
            const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
            const y = gridOriginY + r * offsetY - 50;
            drawNeutralHexagon(ctx, x, y, hexRadius);
        });

        // Dessiner les images
        const imagePromises = hexPositions.map(async ([q, r], index) => {
            const tileImage = tiles[index];

            if (!tileImage || tileImage === '' || tileImage.includes('000')) {
                return;
            }

            const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
            const y = gridOriginY + r * offsetY - 50;

            try {
                const img = await loadImageCached(tileImage);
                const scaledWidth = img.width / 2.4;
                const scaledHeight = img.height / 2.4;

                ctx.drawImage(
                    img,
                    x - scaledWidth / 2,
                    y - scaledHeight / 2,
                    scaledWidth,
                    scaledHeight
                );

                // Redessiner le contour
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
                ctx.strokeStyle = '#666666';
                ctx.lineWidth = 1;
                ctx.stroke();
            } catch (e) {
                // Silencieux
            }
        });

        await Promise.all(imagePromises);
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
     * 🎯 CREATEEFFECT AVEC SIGNAL POUR PERSISTANCE
     */
    let isDrawing = false;
    let redrawTimeout: ReturnType<typeof setTimeout> | undefined;
    const [lastContentKey, setLastContentKey] = createSignal('');

    createEffect(() => {
        // ✅ TRACK SEULEMENT LE MEMO STABLE
        const tilesData = stableTilesData();
        const currentKey = lastContentKey();

        // ✅ SKIP SI LE CONTENU N'A PAS VRAIMENT CHANGÉ
        if (tilesData.key === currentKey || isDrawing || !canvasRef) {
            return;
        }

        console.log('🎨 REAL CONTENT CHANGE', {
            old: currentKey,
            new: tilesData.key,
            tilesCount: tilesData.realTiles.length
        });

        // ✅ METTRE À JOUR LE SIGNAL IMMÉDIATEMENT
        setLastContentKey(tilesData.key);

        // ✅ SIMPLE DEBOUNCE
        if (redrawTimeout) {
            clearTimeout(redrawTimeout);
        }

        redrawTimeout = setTimeout(() => {
            if (!canvasRef || isDrawing) return;

            isDrawing = true;
            const ctx = canvasRef.getContext('2d');
            if (ctx) {
                drawHexagonalGrid(ctx, canvasRef, tilesData.tiles).finally(() => {
                    isDrawing = false;
                });
            } else {
                isDrawing = false;
            }
        }, 50);
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