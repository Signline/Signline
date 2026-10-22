import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';

// Register <signline-form> custom element for embed compatibility if not already defined
if (typeof window !== 'undefined' && 'customElements' in window && !customElements.get('signline-form')) {
  class SignlineFormElement extends HTMLElement {
    connectedCallback() {
      const src = this.getAttribute('data-src') || this.getAttribute('src') || '';
      if (!src) return;
      const iframe = document.createElement('iframe');
      iframe.src = src;
      iframe.style.width = '100%';
      iframe.style.minHeight = '720px';
      iframe.style.border = 'none';
      this.innerHTML = '';
      this.appendChild(iframe);
    }
  }
  customElements.define('signline-form', SignlineFormElement);
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
