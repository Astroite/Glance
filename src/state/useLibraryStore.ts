import { create } from 'zustand';
import { listLibraries, addLibrary, scanLibrary } from '../ipc';
import type { Library } from '../ipc';

interface LibraryState {
  libraries: Library[];
  selectedLibrary: Library | null;
  isLoading: boolean;
  error: string | null;

  loadLibraries: () => Promise<void>;
  selectLibrary: (library: Library) => void;
  addNewLibrary: (path: string) => Promise<void>;
  startScan: (libraryId: number) => Promise<void>;
}

export const useLibraryStore = create<LibraryState>((set, get) => ({
  libraries: [],
  selectedLibrary: null,
  isLoading: false,
  error: null,

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
    set({ isLoading: true, error: null });
    try {
      await scanLibrary(libraryId);
      set({ isLoading: false });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },
}));
