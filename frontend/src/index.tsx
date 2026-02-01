// src/index.tsx - MVU Architecture
import { render } from 'solid-js/web';
import AppMVU from "./components/AppMVU";

const root = document.getElementById('root');

if (import.meta.env.DEV && !(root instanceof HTMLElement)) {
  throw new Error(
      'Root element not found. Did you forget to add it to your index.html? Or maybe the id attribute got misspelled?',
  );
}

// Render with MVU architecture
render(() => <AppMVU />, root!);