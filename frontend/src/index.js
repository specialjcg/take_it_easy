"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
// src/index.tsx
var web_1 = require("solid-js/web");
var App_1 = require("./components/App");
var root = document.getElementById('root');
if (import.meta.env.DEV && !(root instanceof HTMLElement)) {
    throw new Error('Root element not found. Did you forget to add it to your index.html? Or maybe the id attribute got misspelled?');
}
(0, web_1.render)(function () { return <App_1.default />; }, root);
