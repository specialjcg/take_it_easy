// GameModeSelector.tsx - Page de sélection des modes de jeu
import { Component, createSignal } from 'solid-js';
import './ui/styles/GameModeSelector.css';

export interface GameMode {
  id: string;
  name: string;
  description: string;
  simulations?: number;
  icon: string;
  difficulty?: 'Facile' | 'Normal' | 'Difficile';
}

interface GameModeSelectorProps {
  onModeSelected: (mode: GameMode) => void;
}

const GameModeSelector: Component<GameModeSelectorProps> = (props) => {
  const [selectedMode, setSelectedMode] = createSignal<GameMode | null>(null);

  const gameModes: GameMode[] = [
    {
      id: 'single-player-fast',
      name: 'Solo Rapide',
      description: 'Affrontez un MCTS rapide pour des parties courtes',
      simulations: 50,
      icon: '⚡',
      difficulty: 'Facile'
    },
    {
      id: 'single-player',
      name: 'Solo Normal',
      description: 'Mode classique contre un MCTS équilibré',
      simulations: 300,
      icon: '🎮',
      difficulty: 'Normal'
    },
    {
      id: 'single-player-strong',
      name: 'Solo Expert',
      description: 'Défiez un MCTS très fort pour un vrai challenge',
      simulations: 1000,
      icon: '🥊',
      difficulty: 'Difficile'
    },
    {
      id: 'multiplayer',
      name: 'Multijoueur',
      description: 'Jouez avec d\'autres joueurs et un MCTS',
      simulations: 150,
      icon: '👥',
      difficulty: 'Normal'
    },
    {
      id: 'training',
      name: 'Entraînement',
      description: 'Mode d\'entraînement pour améliorer l\'IA',
      icon: '🎓'
    }
  ];

  const handleModeClick = (mode: GameMode) => {
    setSelectedMode(mode);
  };

  const handleStartGame = () => {
    const mode = selectedMode();
    if (mode) {
      props.onModeSelected(mode);
    }
  };

  const getDifficultyClass = (difficulty?: string) => {
    switch (difficulty) {
      case 'Facile': return 'difficulty-easy';
      case 'Normal': return 'difficulty-normal';
      case 'Difficile': return 'difficulty-hard';
      default: return '';
    }
  };

  return (
    <div class="game-mode-selector">
      <div class="header">
        <h1>🎮 Take It Easy</h1>
        <p>Choisissez votre mode de jeu</p>
      </div>

      <div class="modes-grid">
        {gameModes.map((mode) => (
          <div
            class={`mode-card ${selectedMode()?.id === mode.id ? 'selected' : ''}`}
            onClick={() => handleModeClick(mode)}
          >
            <div class="mode-icon">{mode.icon}</div>
            <h3>{mode.name}</h3>
            <p class="mode-description">{mode.description}</p>

            {mode.simulations && (
              <div class="mode-details">
                <span class="simulations">
                  🧠 {mode.simulations} simulations MCTS
                </span>
              </div>
            )}

            {mode.difficulty && (
              <div class={`difficulty-badge ${getDifficultyClass(mode.difficulty)}`}>
                {mode.difficulty}
              </div>
            )}
          </div>
        ))}
      </div>

      {selectedMode() && (
        <div class="action-panel">
          <div class="selected-mode-info">
            <h3>
              {selectedMode()!.icon} {selectedMode()!.name}
            </h3>
            <p>{selectedMode()!.description}</p>
            {selectedMode()!.simulations && (
              <p class="tech-info">
                Puissance MCTS : {selectedMode()!.simulations} simulations par coup
              </p>
            )}
          </div>

          <button
            class="start-button"
            onClick={handleStartGame}
          >
            Commencer la partie
            <span class="start-icon">🚀</span>
          </button>
        </div>
      )}
    </div>
  );
};

export default GameModeSelector;