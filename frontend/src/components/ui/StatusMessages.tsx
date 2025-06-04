// components/ui/StatusMessages.tsx - Affichage des messages d'état et d'erreur
import { Component, Show } from 'solid-js';

interface StatusMessagesProps {
    error: () => string;
    statusMessage: () => string;
}

/**
 * Composant pour l'affichage des messages d'erreur et de statut
 * Simple mais permet de centraliser le styling et la logique d'affichage
 */
export const StatusMessages: Component<StatusMessagesProps> = (props) => {
    return (
        <>
            <Show when={props.error()}>
                <div class="error-message">{props.error()}</div>
            </Show>

            <Show when={props.statusMessage()}>
                <div class="status-message">{props.statusMessage()}</div>
            </Show>
        </>
    );
};