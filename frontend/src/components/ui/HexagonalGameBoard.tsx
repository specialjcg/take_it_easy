// components/ui/HexagonalGameBoard.tsx - Version optimisée
import { Component, createEffect, createSignal, onCleanup, Show } from 'solid-js';

interface HexagonalGameBoardProps {
    plateauTiles: () => {[playerId: string]: string[]};
    availablePositions: () => number[];
    myTurn: () => boolean;
    session: () => { playerId: string } | null;
    onTileClick: (position: number) => void;
}

export const HexagonalGameBoard: Component<HexagonalGameBoardProps> = (props) => {
    let canvasRef: HTMLCanvasElement | undefined;

    // 🚀 CACHE D'IMAGES POUR ÉVITER LES RECHARGEMENTS
    const [imageCache, setImageCache] = createSignal<Map<string, HTMLImageElement>>(new Map());

    // 🚀 ÉTAT PRÉCÉDENT POUR ÉVITER LES REDRAWS INUTILES
    const [lastDrawState, setLastDrawState] = createSignal<string>('');

    // Positions hexagonales exactes du plateau Take It Easy
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
     * 🚀 CACHE D'IMAGES OPTIMISÉ
     */
    const loadImageCached = (src: string): Promise<HTMLImageElement> => {
        return new Promise((resolve, reject) => {
            // ✅ SKIP pour chaînes vides ou images 000.png
            if (!src || src === '' || src.includes('000.png')) {
                // Créer une image vide fictive
                const emptyImg = new Image();
                emptyImg.width = 1;
                emptyImg.height = 1;
                resolve(emptyImg);
                return;
            }

            const cache = imageCache();

            // Vérifier si l'image est déjà en cache
            if (cache.has(src)) {
                resolve(cache.get(src)!);
                return;
            }

            // Charger l'image une seule fois
            const img = new Image();
            img.onload = () => {
                // Ajouter au cache
                const newCache = new Map(cache);
                newCache.set(src, img);
                setImageCache(newCache);
                resolve(img);
            };
            img.onerror = (error) => {
                // ✅ GÉRER l'erreur gracieusement au lieu de rejeter
                console.warn(`⚠️ Image non trouvée: ${src}`);
                // Retourner image vide au lieu d'erreur
                const emptyImg = new Image();
                emptyImg.width = 1;
                emptyImg.height = 1;
                resolve(emptyImg);
            };
            img.src = src;
        });
    };

    /**
     * Dessiner un hexagone individuel
     */
    const drawHexagon = (ctx: CanvasRenderingContext2D, x: number, y: number, radius: number, fillColor?: string) => {
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

        // Remplir l'hexagone si une couleur est fournie
        if (fillColor) {
            ctx.fillStyle = fillColor;
            ctx.fill();
        }

        // Contour blanc
        ctx.strokeStyle = 'white';
        ctx.lineWidth = 2;
        ctx.stroke();
    };

    /**
     * Calculer la distance entre un point et le centre d'un hexagone
     */
    const isPointInHexagon = (pointX: number, pointY: number, hexX: number, hexY: number, radius: number): boolean => {
        const dx = pointX - hexX;
        const dy = pointY - hexY;
        return Math.sqrt(dx * dx + dy * dy) < radius;
    };

    /**
     * 🚀 GÉNERER UN HASH DE L'ÉTAT POUR ÉVITER LES REDRAWS INUTILES
     */
    const generateStateHash = (): string => {
        const currentSession = props.session();
        const playerTiles = currentSession ? props.plateauTiles()[currentSession.playerId] || [] : [];
        const availablePos = props.availablePositions();
        const isMyTurn = props.myTurn();

        return JSON.stringify({
            tiles: playerTiles,
            available: availablePos.toSorted((a, b) => (a - b)),
            myTurn: isMyTurn,
            playerId: currentSession?.playerId
        });
    };
    /**
     * 🚀 DESSINER LE PLATEAU (OPTIMISÉ)
     */
    const drawHexagonalGrid = async (ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement) => {
        // Vérifier si on a besoin de redessiner
        const newStateHash = generateStateHash();
        if (newStateHash === lastDrawState()) {
            return; // Pas de changement, pas de redraw
        }

        const startTime = performance.now();

        // Effacer le canvas
        ctx.clearRect(0, 0, canvas.width, canvas.height);

        // Fond noir
        ctx.fillStyle = '#1e1e1e';
        ctx.fillRect(0, 0, canvas.width, canvas.height);

        // Calculer l'origine du plateau
        const gridOriginX = canvas.width / 2 - hexWidth;
        const gridOriginY = canvas.height / 2 - 2 * offsetY;

        // Obtenir les données actuelles
        const currentSession = props.session();
        const allPlateaus = props.plateauTiles();
        const availablePos = props.availablePositions();
        const isMyTurn = props.myTurn();
        const isViewerMode = currentSession && currentSession.playerId.includes('viewer');

        // 🔧 NOUVEAU: Logique claire pour le plateau à afficher
        let playerTiles: string[] = [];
        let displayMode = '';

        if (isViewerMode) {
            // Mode viewer : SEULEMENT le plateau MCTS
            playerTiles = allPlateaus['mcts_ai'] || [];
            displayMode = 'MCTS Viewer';
        } else {
            // Mode normal : plateau du joueur actuel
            playerTiles = currentSession ? allPlateaus[currentSession.playerId] || [] : [];
            displayMode = 'Player';
        }

        // Dessiner les hexagones avec couleurs unifiées
        hexPositions.forEach(([q, r], index) => {
            const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
            const y = gridOriginY + r * offsetY - 50;

            // 🔧 COULEURS SIMPLIFIÉES ET COHÉRENTES
            let fillColor: string | undefined;

            if (playerTiles[index] && playerTiles[index] !== '') {
                // ✅ TUILE PLACÉE - même couleur partout
                fillColor = isViewerMode
                    ? 'rgba(139, 92, 246, 0.3)'  // Violet pour MCTS viewer
                    : 'rgba(34, 197, 94, 0.3)';  // Vert pour joueur normal
            } else if (!isViewerMode && availablePos.includes(index) && isMyTurn) {
                // ✅ POSITION DISPONIBLE - seulement pour le joueur actif
                fillColor = 'rgba(0, 255, 255, 0.3)'; // Cyan
            }
            // Sinon pas de couleur de fond (case vide)

            // Dessiner l'hexagone
            drawHexagon(ctx, x, y, hexRadius, fillColor);

            // 🔧 LABELS SIMPLIFIÉS
            if (isViewerMode && playerTiles[index] && playerTiles[index] !== '') {
                // Label pour tuiles MCTS placées
                ctx.fillStyle = 'rgba(255, 255, 255, 0.9)';
                ctx.font = 'bold 12px Arial';
                ctx.textAlign = 'center';
                ctx.fillText('🤖', x, y + 4);
            } else if (!isViewerMode && availablePos.includes(index) && isMyTurn) {
                // Numéro pour positions disponibles
                ctx.fillStyle = 'rgba(255, 255, 255, 0.8)';
                ctx.font = '10px Arial';
                ctx.textAlign = 'center';
                ctx.fillText(index.toString(), x, y + 3);
            }
        });

        // 🚀 CHARGER ET DESSINER LES IMAGES EN PARALLÈLE
        const imagePromises: Promise<void>[] = [];

        hexPositions.forEach(([q, r], index) => {
            if (playerTiles[index] && playerTiles[index] !== '') {
                const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
                const y = gridOriginY + r * offsetY - 50;

                const imagePromise = loadImageCached(playerTiles[index])
                    .then(img => {
                        const scaledWidth = img.width / 3;
                        const scaledHeight = img.height / 3;

                        // Dessiner l'image
                        ctx.drawImage(
                            img,
                            x - scaledWidth / 2,
                            y - scaledHeight / 2,
                            scaledWidth,
                            scaledHeight
                        );

                        // Redessiner le contour par-dessus
                        drawHexagon(ctx, x, y, hexRadius);
                    })
                    .catch(err => {
                        console.warn(`Erreur chargement image ${playerTiles[index]}:`, err);
                    });

                imagePromises.push(imagePromise);
            }
        });

        // Attendre que toutes les images soient chargées
        await Promise.all(imagePromises);

        // Sauvegarder le nouvel état
        setLastDrawState(newStateHash);

        const endTime = performance.now();
    };

    /**
     * 🚀 GESTION OPTIMISÉE DES CLICS (AVEC DEBOUNCE)
     */
    let clickTimeout: ReturnType<typeof setTimeout> | undefined;

    const handleCanvasClick = (e: MouseEvent) => {
        const currentSession = props.session();
        const isViewerMode = currentSession && currentSession.playerId.includes('viewer');

        if (isViewerMode) {
            return;
        }
        // Debounce pour éviter les clics multiples
        if (clickTimeout) {
            clearTimeout(clickTimeout);
        }

        clickTimeout = setTimeout(() => {
            if (!canvasRef || !props.myTurn()) {
                return;
            }

            const currentSession = props.session();
            if (!currentSession) {
                return;
            }

            const rect = canvasRef.getBoundingClientRect();
            const clickX = e.clientX - rect.left;
            const clickY = e.clientY - rect.top;

            const gridOriginX = canvasRef.width / 2 - hexWidth;
            const gridOriginY = canvasRef.height / 2 - 2 * offsetY;

            // Chercher quel hexagone a été cliqué
            for (let index = 0; index < hexPositions.length; index++) {
                const [q, r] = hexPositions[index];
                const x = gridOriginX + q * hexWidth + r * (hexWidth / 6) + 50;
                const y = gridOriginY + r * offsetY - 50;

                if (isPointInHexagon(clickX, clickY, x, y, hexRadius)) {
                    if (props.availablePositions().includes(index)) {
                        props.onTileClick(index);
                    } else {
                    }
                    return;
                }
            }

        }, 100); // Debounce de 100ms
    };

    // 🚀 EFFET OPTIMISÉ AVEC VÉRIFICATION DE CHANGEMENT
    createEffect(() => {
        if (canvasRef) {
            const ctx = canvasRef.getContext('2d');
            if (ctx) {
                drawHexagonalGrid(ctx, canvasRef);
            }
        }
    });

    // Nettoyer le timeout à la destruction
    onCleanup(() => {
        if (clickTimeout) {
            clearTimeout(clickTimeout);
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
                    cursor: props.myTurn() && !props.session()?.playerId.includes('viewer') ? 'pointer' : 'default'
                }}
            />

            {/* Instructions adaptées */}
            <div class="classic-instructions">
                <Show when={props.session()?.playerId.includes('viewer')}>
                    <p style={{ color: '#8b5cf6', 'font-weight': 'bold' }}>
                        👁️ Mode Observateur - Plateau MCTS affiché
                    </p>
                    <p style={{ color: '#666', 'font-size': '0.8em' }}>
                        Les tuiles violettes montrent les mouvements de l'IA
                    </p>
                </Show>

                <Show when={!props.session()?.playerId.includes('viewer')}>
                    {/* Instructions normales existantes */}
                    <Show when={props.myTurn() && props.availablePositions().length > 0}>
                        <p style={{ color: '#00ffff', 'font-weight': 'bold' }}>
                            ✨ Cliquez sur un hexagone cyan pour placer votre tuile
                        </p>
                    </Show>
                </Show>
            </div>
        </div>
    );
};