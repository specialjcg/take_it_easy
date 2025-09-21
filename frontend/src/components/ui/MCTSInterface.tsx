// components/ui/MCTSInterface.tsx - Interface spécialisée MCTS
import { Component, Show, createMemo } from 'solid-js';
import { useGameState } from '../../hooks/useGameState';

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
    const mctsInfo = createMemo(() => {
        const state = gameState.gameState();
        const mctsPlayer = state?.players?.find(p => p.id === 'mcts_ai');
        return {
            name: mctsPlayer?.name || '🤖 MCTS IA',
            score: mctsPlayer?.score || 0,
            isReady: mctsPlayer?.isReady || false,
            isConnected: mctsPlayer?.isConnected || false
        };
    });

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
                    <span>Score: <strong>{mctsInfo().score} points</strong></span>
                    <span class={mctsInfo().isConnected ? 'status-connected' : 'status-disconnected'}>
                        {mctsInfo().isConnected ? '🟢 Connecté' : '🔴 Déconnecté'}
                    </span>
                </div>
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
                <Show when={props.myTurn()}>
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
                <Show when={!props.myTurn()}>
                    <div class="mcts-waiting">
                        <span>⏳ En attente du tour de MCTS...</span>
                    </div>
                </Show>
            </div>

            {/* Afficher le plateau avec focus MCTS */}
            {props.renderGameBoard()}
        </div>
    );
};