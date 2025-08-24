// hooks/useLoadingManager.ts - Loading state centralisé
import { createSignal, createMemo, batch } from 'solid-js';

/**
 * Hook pour gestion centralisée des états de loading
 * Évite les flashs visuels et les états incohérents
 */
export const useLoadingManager = () => {
    const [loadingStates, setLoadingStates] = createSignal<Record<string, boolean>>({});

    // Memo pour savoir si quelque chose est en loading
    const isAnyLoading = createMemo(() => {
        const states = loadingStates();
        return Object.values(states).some(loading => loading);
    });

    // Vérifier si une action spécifique est en loading
    const isLoadingSpecific = (key: string) => createMemo(() => loadingStates()[key] || false);

    // Définir l'état loading d'une action spécifique
    const setLoading = (key: string, loading: boolean) => {
        batch(() => {
            setLoadingStates(prev => ({
                ...prev,
                [key]: loading
            }));
        });
    };

    // Wrapper pour exécuter une action avec loading automatique
    const withLoading = async <T>(
        key: string,
        action: () => Promise<T>
    ): Promise<T> => {
        setLoading(key, true);
        try {
            const result = await action();
            return result;
        } finally {
            setLoading(key, false);
        }
    };

    // Réinitialiser tous les états
    const clearAll = () => {
        setLoadingStates({});
    };

    // Obtenir la liste des actions actives
    const getActiveLoadings = () => {
        const states = loadingStates();
        return Object.keys(states).filter(key => states[key]);
    };

    // Obtenir un message de loading contextuel
    const getLoadingMessage = () => createMemo(() => {
        const active = getActiveLoadings();
        if (active.length === 0) return '';
        
        const messages: Record<string, string> = {
            'create-session': 'Création de session...',
            'join-session': 'Connexion...',
            'set-ready': 'Validation...',
            'start-turn': 'Nouveau tour...',
            'play-move': 'Placement de tuile...',
            'leave-session': 'Déconnexion...'
        };
        
        return messages[active[0]] || 'Chargement...';
    });

    return {
        // États
        isAnyLoading,
        isLoadingSpecific,
        getLoadingMessage,
        
        // Actions
        setLoading,
        withLoading,
        clearAll,
        
        // Debug
        getActiveLoadings,
        getDebugInfo: () => ({
            states: loadingStates(),
            active: getActiveLoadings(),
            count: getActiveLoadings().length
        })
    };
};