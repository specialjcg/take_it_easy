// App.tsx - Composant principal avec sélection de mode
import { Component, createSignal, Show, onMount } from 'solid-js';
import GameModeSelector, { GameMode } from './GameModeSelector';
import MultiplayerApp from './MultiplayerApp';

const App: Component = () => {
  const [selectedMode, setSelectedMode] = createSignal<GameMode | null>(null);
  const [autoConnectSolo, setAutoConnectSolo] = createSignal<boolean>(false);

  // Détecter le mode viewer au démarrage
  onMount(() => {
    const urlParams = new URLSearchParams(window.location.search);
    const mode = urlParams.get('mode');

    // Si on est en mode viewer/mcts_view, définir un mode par défaut
    if (mode === 'mcts_view' || mode === 'viewer') {
      console.log('🔍 Mode viewer détecté au démarrage, définition du mode par défaut');
      const viewerMode: GameMode = {
        id: 'viewer-mode',
        name: 'MCTS Viewer',
        description: 'Mode observation des parties MCTS',
        icon: '👁️'
      };
      setSelectedMode(viewerMode);
      setAutoConnectSolo(false); // Pas d'auto-connexion pour les viewers
    }
  });

  const handleModeSelected = (mode: GameMode) => {
    console.log('🎮 Mode sélectionné:', mode.id, '-', mode.name);
    setSelectedMode(mode);


    // Auto-connexion pour les modes solo
    if (mode.id.startsWith('single-player') || mode.id === 'training') {
      console.log('🤖 Mode solo détecté - auto-connexion activée');
      setAutoConnectSolo(true);
    } else {
      setAutoConnectSolo(false);
    }
  };

  const handleBackToModeSelection = () => {
    setSelectedMode(null);
    setAutoConnectSolo(false);
  };

  return (
    <div>
      <Show when={!selectedMode()}>
        <GameModeSelector onModeSelected={handleModeSelected} />
      </Show>

      <Show when={selectedMode()}>
        <MultiplayerApp
          gameMode={selectedMode()!}
          autoConnectSolo={autoConnectSolo()}
          onBackToModeSelection={handleBackToModeSelection}
        />
      </Show>
    </div>
  );
};

export default App;