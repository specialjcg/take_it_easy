// src/index.tsx
import { render } from 'solid-js/web';
import MultiplayerApp from "./components/MultiplayerApp";

const root = document.getElementById('root');

if (import.meta.env.DEV && !(root instanceof HTMLElement)) {
  throw new Error(
      'Root element not found. Did you forget to add it to your index.html? Or maybe the id attribute got misspelled?',
  );
}

render(() => <MultiplayerApp />, root!);