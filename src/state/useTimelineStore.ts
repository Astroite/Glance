import { create } from 'zustand';
import { queryTimeline } from '../ipc';
import type { PhotoSummary, TimelinePage } from '../ipc';

interface TimelineState {
  photos: PhotoSummary[];
  nextCursor: string | null;
  isLoading: boolean;
  hasMore: boolean;
  error: string | null;
  viewMode: 'waterfall' | 'grid';

  loadInitial: (libraryId: number) => Promise<void>;
  loadMore: (libraryId: number) => Promise<void>;
  setViewMode: (mode: 'waterfall' | 'grid') => void;
  reset: () => void;
}

export const useTimelineStore = create<TimelineState>((set, get) => ({
  photos: [],
  nextCursor: null,
  isLoading: false,
  hasMore: true,
  error: null,
  viewMode: 'waterfall',

  loadInitial: async (libraryId) => {
    set({ isLoading: true, error: null, photos: [], nextCursor: null, hasMore: true });
    try {
      const page: TimelinePage = await queryTimeline(libraryId, null);
      set({
        photos: page.photos,
        nextCursor: page.next_cursor,
        hasMore: page.next_cursor !== null,
        isLoading: false,
      });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  loadMore: async (libraryId) => {
    const { nextCursor, isLoading, photos } = get();
    if (isLoading || !nextCursor) return;

    set({ isLoading: true });
    try {
      const page: TimelinePage = await queryTimeline(libraryId, nextCursor);
      set({
        photos: [...photos, ...page.photos],
        nextCursor: page.next_cursor,
        hasMore: page.next_cursor !== null,
        isLoading: false,
      });
    } catch (err) {
      set({ error: String(err), isLoading: false });
    }
  },

  setViewMode: (mode) => {
    set({ viewMode: mode });
  },

  reset: () => {
    set({ photos: [], nextCursor: null, hasMore: true, error: null });
  },
}));
