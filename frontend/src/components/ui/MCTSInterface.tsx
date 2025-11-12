// components/ui/MCTSInterface.tsx - Interface spécialisée MCTS
import { Component, For, Show, createMemo } from 'solid-js';
import { useGameState } from '../../hooks/useGameState';
import { SessionState } from '../../generated/common';

interface MCTSInterfaceProps {
    sessionCode: () => string;
    myTurn: () => boolean;
    renderGameBoard: () => any; // JSX element
}

/**
 * Interface spécialisée pour l'affichage MCTS
 * Composant dédié pour une expérience utilisateur différenciée
 */
// Dans MCTSInterface.tsx - AMÉLIORER avec info plateau MCTS
export const MCTSInterface: Component<MCTSInterfaceProps> = (props) => {
    const gameState = useGameState();

    // Informations sur le MCTS depuis le state de la session
    const state = createMemo(() => gameState.gameState());
    const finalScores = gameState.finalScores;

    const derivedPlayers = createMemo(() => {
        const players = state()?.players;
        if (players && players.length) {
            return players;
        }
        const scores = finalScores();
        if (!scores) return [];
        return Object.entries(scores).map(([id, score]) => ({
            id,
            name: id === 'mcts_ai' ? '🤖 MCTS IA' : `Joueur ${id.slice(0, 4)}`,
            score,
            isReady: true,
            isConnected: true,
            joinedAt: ''
        }));
    });

    const sortedPlayers = createMemo(() => {
        const players = derivedPlayers();
        return [...players].sort((a, b) => (b.score ?? 0) - (a.score ?? 0));
    });

    const mctsInfo = createMemo(() => {
        const mctsPlayer =
            sortedPlayers().find((p) => p.id === 'mcts_ai') ||
            sortedPlayers().find((p) => p.name?.toLowerCase().includes('mcts'));

        return {
            name: mctsPlayer?.name ?? '🤖 MCTS IA',
            score:
                typeof mctsPlayer?.score === 'number'
                    ? mctsPlayer.score
                    : finalScores()?.['mcts_ai'] ?? null,
            isConnected: mctsPlayer?.isConnected ?? true,
        };
    });

    const isMctsTurn = createMemo(() => state()?.currentTurn === 'mcts_ai');

    const gameFinished = createMemo(() => state()?.state === SessionState.FINISHED);

    return (
        <div class="mcts-interface">
            <div class="mcts-header">
                <h1>👁️ MCTS Observer</h1>
                <div class="mcts-session-info">
                    <span>Session: <strong>{props.sessionCode()}</strong></span>
                    <span>Mode: <strong>🤖 Plateau MCTS IA</strong></span>
                </div>
                <div class="mcts-player-info">
                    <span>Joueur: <strong>{mctsInfo().name}</strong></span>
                    <Show when={mctsInfo().score !== null} fallback={<span>Score: <strong>…</strong></span>}>
                        <span>Score: <strong>{mctsInfo().score} points</strong></span>
                    </Show>
                    <span class={mctsInfo().isConnected ? 'status-connected' : 'status-disconnected'}>
                        {mctsInfo().isConnected ? '🟢 Connecté' : '🔴 Déconnecté'}
                    </span>
                </div>
            </div>
            <div class="mcts-scoreboard glass-container">
                <h3>🏆 Scores en temps réel</h3>
                <Show when={sortedPlayers().length > 0} fallback={<p>Aucun score disponible pour le moment.</p>}>
                    <For each={sortedPlayers()}>
                        {(player) => (
                            <div
                                class={`score-item ${
                                    player.id === 'mcts_ai' ? 'player-score-ai' : ''
                                } ${player.id === gameState.session()?.playerId ? 'player-score-self' : ''}`}
                            >
                                <span class="player-name">
                                    {player.id === 'mcts_ai' ? '🤖 IA' : player.name}
                                </span>
                                <span class="player-score">{player.score} pts</span>
                            </div>
                        )}
                    </For>
                </Show>
            </div>

            <div class="viewer-info">
                <div class="viewer-status">
                    <span class="viewer-icon">🤖</span>
                    <span>Vous observez les mouvements de l'IA MCTS</span>
                </div>
                <div class="viewer-note">
                    <small>Les tuiles violettes montrent où MCTS a joué ses coups</small>
                </div>
            </div>

            <div class="mcts-status">
                <Show when={isMctsTurn()}>
                    <div class="mcts-thinking">
                        <span class="thinking-icon">🧠</span>
                        <span>MCTS calcule le meilleur mouvement...</span>
                        <div class="thinking-animation">
                            <div class="dot"></div>
                            <div class="dot"></div>
                            <div class="dot"></div>
                        </div>
                    </div>
                </Show>
                <Show when={!isMctsTurn()}>
                    <div class="mcts-waiting">
                        <span>
                            {gameFinished()
                                ? '✅ Partie terminée'
                                : '⏳ En attente du tour de MCTS...'}
                        </span>
                    </div>
                </Show>
            </div>

            {/* Afficher le plateau avec focus MCTS */}
            {props.renderGameBoard()}
        </div>
    );
};
