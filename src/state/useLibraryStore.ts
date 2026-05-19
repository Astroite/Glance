import { create } from 'zustand';
import {
  listLibraries,
  addLibrary,
  scanLibrary,
  pauseScan,
  resumeScan,
  onScanComplete,
  onScanPaused,
  onScanError,
} from '../ipc';
import type { Library } from '../ipc';

interface LibraryState {
  libraries: Library[];
  selectedLibrary: Library | null;
  isLoading: boolean;
  error: string | null;
  scanningLibraries: Set<number>;

  loadLibraries: () => Promise<void>;
  selectLibrary: (library: Library) => void;
  addNewLibrary: (path: string) => Promise<void>;
  startScan: (libraryId: number) => Promise<void>;
  pauseScan: (libraryId: number) => Promise<void>;
  resumeScan: (libraryId: number) => Promise<void>;
  _setupListeners: () => void;
}

let listenersInitialized = false;

export const useLibraryStore = create<LibraryState>((set, get) => ({
  libraries: [],
  selectedLibrary: null,
  isLoading: false,
  error: null,
  scanningLibraries: new Set(),

  loadLibraries: async () => {
    set({ isLoading: true, error: null });
    try {
      const libraries = await listLibraries();
      set({ libraries, isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  selectLibrary: (library) => {
    set({ selectedLibrary: library });
  },

  addNewLibrary: async (path) => {
    set({ isLoading: true, error: null });
    try {
      const library = await addLibrary(path);
      const { libraries } = get();
      set({
        libraries: [...libraries, library],
        selectedLibrary: library,
        isLoading: false,
      });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  startScan: async (libraryId) => {
    try {
      await scanLibrary(libraryId);
      set((state) => {
        const next = new Set(state.scanningLibraries);
        next.add(libraryId);
        return { scanningLibraries: next, error: null };
      });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  pauseScan: async (libraryId) => {
    try {
      await pauseScan(libraryId);
    } catch (err) {
      set({ error: String(err) });
    }
  },

  resumeScan: async (libraryId) => {
    try {
      await resumeScan(libraryId);
      set((state) => {
        const next = new Set(state.scanningLibraries);
        next.add(libraryId);
        return { scanningLibraries: next, error: null };
      });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  _setupListeners: () => {
    if (listenersInitialized) return;
    listenersInitialized = true;

    onScanComplete((payload) => {
      set((state) => {
        const next = new Set(state.scanningLibraries);
        next.delete(payload.library_id);
        return { scanningLibraries: next };
      });
    });

    onScanPaused((payload) => {
      set((state) => {
        const next = new Set(state.scanningLibraries);
        next.delete(payload.library_id);
        return { scanningLibraries: next };
      });
    });

    onScanError((payload) => {
      set((state) => {
        const next = new Set(state.scanningLibraries);
        next.delete(payload.library_id);
        return { scanningLibraries: next, error: payload.error };
      });
    });
  },
}));
