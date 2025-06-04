// components/ui/ConnectionInterface.tsx - Interface de connexion/création de session
import { Component, Show } from 'solid-js';

interface ConnectionInterfaceProps {
    playerName: () => string;
    setPlayerName: (name: string) => void;
    sessionCode: () => string;
    setSessionCode: (code: string) => void;
    loading: () => boolean;
    onCreateSession: () => void;
    onJoinSession: () => void;
}

/**
 * Composant d'interface pour la connexion et création de sessions
 * Extrait du composant principal pour améliorer la réutilisabilité
 */
export const ConnectionInterface: Component<ConnectionInterfaceProps> = (props) => {
    return (
        <div class="connection-section">
            <div class="input-group">
                <label for="player-name">Nom du joueur :</label>
                <input
                    id="player-name"
                    type="text"
                    class="player-name-input"
                    value={props.playerName()}
                    onInput={(e) => props.setPlayerName(e.target.value)}
                    placeholder="Entrez votre nom"
                    maxLength={20}
                />
            </div>

            <div class="actions">
                <button
                    onClick={props.onCreateSession}
                    disabled={props.loading()}
                    class="create-button"
                >
                    {props.loading() ? 'Création...' : 'Créer une nouvelle session'}
                </button>

                <div class="join-section">
                    <input
                        type="text"
                        class="session-code-input"
                        value={props.sessionCode()}
                        onInput={(e) => props.setSessionCode(e.target.value.toUpperCase())}
                        placeholder="CODE"
                        maxLength={6}
                    />
                    <button
                        onClick={props.onJoinSession}
                        disabled={props.loading()}
                        class="join-button"
                    >
                        {props.loading() ? 'Connexion...' : 'Rejoindre'}
                    </button>
                </div>
            </div>
        </div>
    );
};