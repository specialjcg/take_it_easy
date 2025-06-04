// components/ui/CurrentTileDisplay.tsx - Affichage de la tuile courante
import { Component, Show } from 'solid-js';

interface CurrentTileDisplayProps {
    currentTile: () => string | null;
    currentTileImage: () => string | null;
    imageCache: () => string | null;
}

/**
 * Composant pour l'affichage de la tuile courante avec gestion du cache
 * Composant isolé pour une meilleure réutilisabilité
 */
export const CurrentTileDisplay: Component<CurrentTileDisplayProps> = (props) => {
    return (
        <Show when={props.currentTile() && props.currentTileImage()}>
            <div class="current-tile-display-section">
                <h4>🎲 Tuile annoncée</h4>
                <div class="current-tile-container">
                    <Show when={props.imageCache()}>
                        <img
                            src={props.imageCache()!}
                            alt={`Tuile ${props.currentTile()}`}
                            class="current-tile-image"
                            style="opacity: 0; width: 110px; height: 110px; object-fit: cover; transform: scale(5.2); transition: opacity 0.3s ease;"
                            onLoad={(e) => {
                                e.currentTarget.style.opacity = '1';
                            }}
                            onError={(e) => {
                                e.currentTarget.style.border = '4px solid red';
                                e.currentTarget.style.opacity = '1';
                            }}
                        />
                    </Show>
                </div>
            </div>
        </Show>
    );
};