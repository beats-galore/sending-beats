import React, { useCallback, useState } from 'react';

import AdminPanel from './components/AdminPanel';
import DJClient from './components/DJClient';
import ListenerPlayer from './components/ListenerPlayer';
import { VirtualMixerWithErrorBoundary as VirtualMixer } from './components/mixer';
import OldVirtualMixer from './components/VirtualMixer';

const NowPlayingCard: React.FC = () => (
  <div className="bg-surface rounded-2xl shadow-card p-6 flex items-center gap-6 max-w-md w-full mx-auto mb-8">
    <div className="w-20 h-20 bg-brand/80 rounded-xl flex items-center justify-center text-white text-3xl font-display">
      <span role="img" aria-label="music">
        🎵
      </span>
    </div>
    <div className="flex-1">
      <div className="text-brand font-display text-lg font-bold tracking-wide mb-1">
        Now Playing
      </div>
      <div className="text-white font-display text-xl leading-tight">Midnight Groove</div>
      <div className="text-accent text-sm mt-1">DJ Luna</div>
    </div>
    <div className="flex flex-col items-end">
      <span className="text-xs text-surface-light">Live</span>
      <span className="w-2 h-2 bg-accent rounded-full animate-pulse mt-1" />
    </div>
  </div>
);

const App: React.FC = () => {
  const [currentView, setCurrentView] = useState<
    'home' | 'dj' | 'admin' | 'listener' | 'mixer' | 'old-mixer'
  >('mixer');

  console.log('re-rendered app');

  const renderContent = useCallback(() => {
    switch (currentView) {
      case 'mixer':
        return <VirtualMixer />;
      case 'old-mixer':
        return <OldVirtualMixer />;
      case 'dj':
        return <DJClient />;
      case 'admin':
        return <AdminPanel />;
      case 'listener':
        return <ListenerPlayer />;
      default:
        return (
          <div className="flex flex-col items-center justify-center px-4">
            <NowPlayingCard />
            <h2 className="text-xl font-display mb-4 text-brand-light">
              Welcome to the Radio Streaming Platform
            </h2>
            <p className="mb-2 text-surface-light">Choose a section above to get started.</p>
          </div>
        );
    }
  }, [currentView]);

  return (
    <div className="min-h-screen bg-surface-dark text-white flex flex-col font-body">
      <header className="p-4 bg-surface flex items-center justify-between shadow-md">
        <h1 className="text-2xl font-display tracking-tight text-brand">Sendin Beats Radio</h1>
        <nav className="space-x-4">
          <button
            onClick={() => setCurrentView('home')}
            className={`hover:underline ${currentView === 'home' ? 'text-brand' : 'text-surface-light'}`}
          >
            Home
          </button>
          <button
            onClick={() => setCurrentView('mixer')}
            className={`hover:underline ${currentView === 'mixer' ? 'text-brand' : 'text-surface-light'}`}
          >
            Virtual Mixer
          </button>
          <button
            onClick={() => setCurrentView('old-mixer')}
            className={`hover:underline ${currentView === 'old-mixer' ? 'text-brand' : 'text-surface-light'}`}
          >
            Old Mixer
          </button>
          <button
            onClick={() => setCurrentView('dj')}
            className={`hover:underline ${currentView === 'dj' ? 'text-brand' : 'text-surface-light'}`}
          >
            DJ Client
          </button>
          <button
            onClick={() => setCurrentView('listener')}
            className={`hover:underline ${currentView === 'listener' ? 'text-accent' : 'text-surface-light'}`}
          >
            Listen
          </button>
          <button
            onClick={() => setCurrentView('admin')}
            className={`hover:underline ${currentView === 'admin' ? 'text-accent' : 'text-surface-light'}`}
          >
            Admin Panel
          </button>
        </nav>
      </header>
      <main className="flex-1 flex flex-col items-center justify-center px-4 py-8">
        {renderContent()}
      </main>
      <footer className="p-2 bg-surface text-center text-xs text-surface-light">
        &copy; {new Date().getFullYear()} Sendin Beats
      </footer>
    </div>
  );
};

export default App;
