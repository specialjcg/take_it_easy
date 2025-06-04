// components/ui/PlayersList.tsx - Affichage de la liste des joueurs
import { Component, Show, For } from 'solid-js';
import { SessionState } from '../../generated/common';
import type { Player, GameState } from '../../hooks/useGameState';

interface PlayersListProps {
    gameState: () => GameState | null;
    isCurrentPlayer: (playerId: string) => boolean;
    getPlayerStatus: (player: Player) => string;
    isPlayerReady: () => boolean;
    loading: () => boolean;
    onSetReady: () => void;
    onOpenMctsSession: () => void;
    session: () => { playerId: string; sessionCode: string } | null;
}

/**
 * Composant pour l'affichage de la liste des joueurs et contrôles MCTS
 * Extrait du composant principal pour une meilleure modularité
 */
export const PlayersList: Component<PlayersListProps> = (props) => {
    return (
        <div class="players-section glass-container">
            <h3>Joueurs ({props.gameState()?.players.length || 0})</h3>
            
            {/* Contrôles MCTS */}
            <div class="mcts-controls">
                <button
                    class="open-mcts-button"
                    onClick={props.onOpenMctsSession}
                    disabled={!props.session()}
                >
                    🤖 Voir session MCTS
                </button>
            </div>

            <div class="players-list">
                <For each={props.gameState()?.players || []}>
                    {(player) => (
                        <div
                            class={`player-card ${
                                props.isCurrentPlayer(player.id) ? 'current-player' : ''
                            } ${player.id === 'mcts_ai' ? 'mcts-player' : ''}`}
                        >
                            <div class="player-info">
                                <span class="player-name">
                                    {player.id === 'mcts_ai' ? '🤖 MCTS IA' : player.name}
                                    {props.isCurrentPlayer(player.id) && (
                                        <span class="you-indicator"> (Vous)</span>
                                    )}
                                </span>
                                <span class="player-score">Score: {player.score}</span>
                            </div>
                            <div class="player-status">
                                {player.id === 'mcts_ai' ? '🤖 IA' : props.getPlayerStatus(player)}
                            </div>
                        </div>
                    )}
                </For>
            </div>

            <Show when={props.gameState()?.state === SessionState.WAITING}>
                <div class="ready-section">
                    <Show when={!props.isPlayerReady()}>
                        <button
                            onClick={props.onSetReady}
                            disabled={props.loading()}
                            class="ready-button"
                        >
                            Je suis prêt !
                        </button>
                    </Show>
                    <Show when={props.isPlayerReady()}>
                        <div class="ready-status">
                            ✅ Vous êtes prêt ! En attente des autres joueurs...
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    );
};