import './app.css';

import { mount } from 'svelte';

import App from './App.svelte';

const target = document.getElementById('app');

if (target === null) {
    document.body.textContent = 'missing #app root';
} else {
    mount(App, { target });
}
