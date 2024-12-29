# Reinforcement Learning for Tile Placement Game

This project implements a reinforcement learning agent for a tile-placement strategy game. The game involves placing tiles on a hexagonal grid, with scoring based on specific patterns.

## Features

- **Monte Carlo Tree Search (MCTS):** Combines MCTS with a neural network to simulate and find optimal moves.
- **Neural Networks:** Separate policy and value networks for guidance.
- **Dynamic Visualization:** Visualizes the game's progress and learning metrics through a web-based interface.
- **Scoring Logic:** Custom scoring rules based on the game's patterns and mechanics.
- **Test Coverage:** Comprehensive unit tests for core functionalities.

## Getting Started

### Prerequisites

- **Rust Toolchain:** Install Rust from [rust-lang.org](https://www.rust-lang.org/).
- **Python (Optional):** Required if extending with scripts for analysis.
- **WebSocket Library:** `tokio` and `tokio-tungstenite` are used for WebSocket support.
- **Machine Learning Framework:** `tch` for neural network functionalities.


