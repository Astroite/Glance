import { useEffect } from 'react';
import { useLibraryStore } from './state/useLibraryStore';
import { LibrarySetup } from './views/LibrarySetup';
import { Timeline } from './views/Timeline';
import './styles/global.css';

function App() {
  const { selectedLibrary, loadLibraries } = useLibraryStore();

  useEffect(() => {
    loadLibraries();
  }, [loadLibraries]);

  return (
    <div className="app">
      <header className="app-header">
        <h1>Glance</h1>
        <p>A local-first photo timeline for photographers</p>
      </header>

      <main className="app-main">
        {selectedLibrary ? <Timeline /> : <LibrarySetup />}
      </main>
    </div>
  );
}

export default App;
