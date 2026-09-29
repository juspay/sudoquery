import { createContext, useContext, useState, ReactNode } from 'react';

interface HistoryDrawerContextType {
  isOpen: boolean;
  toggle: () => void;
  open: () => void;
  close: () => void;
}

const HistoryDrawerContext = createContext<HistoryDrawerContextType | null>(null);

export function HistoryDrawerProvider({ children }: { children: ReactNode }) {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <HistoryDrawerContext.Provider
      value={{
        isOpen,
        toggle: () => setIsOpen((prev) => !prev),
        open: () => setIsOpen(true),
        close: () => setIsOpen(false),
      }}
    >
      {children}
    </HistoryDrawerContext.Provider>
  );
}

export function useHistoryDrawer() {
  const context = useContext(HistoryDrawerContext);
  if (!context) {
    throw new Error('useHistoryDrawer must be used within a HistoryDrawerProvider');
  }
  return context;
}
