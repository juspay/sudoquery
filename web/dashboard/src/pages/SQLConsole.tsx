import { useState, useCallback, useEffect } from 'react';
import {
  Box,
  Button,
  Paper,
  Typography,
  Alert,
  CircularProgress,
  Tabs,
  Tab,
  IconButton,
} from '@mui/material';
import { Plus as PlusIcon, X as CloseIcon } from 'lucide-react';
import { consoleService } from '../services/consoleService';
import ConsoleEditor from '../components/ConsoleEditor';
import type { UserProjectConsole } from '../types/api';

export default function SQLConsole() {
  const [consoles, setConsoles] = useState<UserProjectConsole[]>([]);
  const [activeConsoleId, setActiveConsoleId] = useState<string>('');
  const [queryByConsole, setQueryByConsole] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loadingConsoleId, setLoadingConsoleId] = useState<string | null>(null);
  const [editingConsoleId, setEditingConsoleId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState('');

  const fetchConsoles = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const data = await consoleService.list();
      setConsoles(data);
      if (data.length > 0) {
        setActiveConsoleId(data[0].id);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to load consoles';
      setLoadError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchConsoles();
  }, [fetchConsoles]);

  useEffect(() => {
    if (!activeConsoleId || activeConsoleId in queryByConsole) return;
    setLoadingConsoleId(activeConsoleId);
    consoleService.get(activeConsoleId)
      .then((console) => {
        setQueryByConsole((prev) => ({ ...prev, [activeConsoleId]: console.console ?? '' }));
      })
      .catch(() => {
        setLoadError('Failed to load console content');
      })
      .finally(() => {
        setLoadingConsoleId(null);
      });
  }, [activeConsoleId]);

  const handleTabChange = async (_: React.SyntheticEvent, newValue: string) => {
    if (editingConsoleId) return;
    setActiveConsoleId(newValue);

    if (!(newValue in queryByConsole)) {
      setLoadingConsoleId(newValue);
      try {
        const console = await consoleService.get(newValue);
        setQueryByConsole((prev) => ({ ...prev, [newValue]: console.console ?? '' }));
      } catch (err) {
        setLoadError('Failed to load console content');
      } finally {
        setLoadingConsoleId(null);
      }
    }
  };

  const handleQueryChange = useCallback((consoleId: string, query: string) => {
    setQueryByConsole((prev) => ({ ...prev, [consoleId]: query }));
  }, []);

  const handleCreateConsole = async () => {
    try {
      const newConsole = await consoleService.create('Untitled', '');
      setConsoles((prev) => [...prev, newConsole]);
      setQueryByConsole((prev) => ({ ...prev, [newConsole.id]: '' }));
      setActiveConsoleId(newConsole.id);
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to create console';
      setLoadError(message);
    }
  };

  const handleDeleteConsole = async (consoleId: string) => {
    try {
      await consoleService.delete(consoleId);
      setConsoles((prev) => {
        const filtered = prev.filter((c) => c.id !== consoleId);
        if (activeConsoleId === consoleId) {
          setActiveConsoleId(filtered[0]?.id ?? '');
        }
        return filtered;
      });
      setQueryByConsole((prev) => {
        const { [consoleId]: _, ...rest } = prev;
        return rest;
      });
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to delete console';
      setLoadError(message);
    }
  };

  const handleStartEditing = (consoleId: string, currentName: string | null) => {
    setEditingConsoleId(consoleId);
    setEditingName(currentName || 'Untitled');
  };

  const handleSaveName = async () => {
    if (!editingConsoleId || !editingName.trim()) {
      setEditingConsoleId(null);
      return;
    }

    try {
      await consoleService.update(editingConsoleId, editingName.trim(), queryByConsole[editingConsoleId] ?? '');
      setConsoles((prev) =>
        prev.map((c) => (c.id === editingConsoleId ? { ...c, name: editingName.trim() } : c))
      );
    } catch (err) {
      setLoadError('Failed to rename console');
    } finally {
      setEditingConsoleId(null);
      setEditingName('');
    }
  };

  const handleNameKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault();
      void handleSaveName();
    } else if (e.key === 'Escape') {
      setEditingConsoleId(null);
      setEditingName('');
    }
  };

  const activeConsole = consoles.find((c) => c.id === activeConsoleId);

  if (loading) {
    return (
      <Box sx={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      {loadError && (
        <Alert severity="error" sx={{ m: 2, borderRadius: 2 }}>
          {loadError}
        </Alert>
      )}

      {consoles.length === 0 ? (
        <Box sx={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <Paper
            elevation={0}
            sx={{
              p: 4,
              textAlign: 'center',
              border: '1px dashed',
              borderColor: 'divider',
              borderRadius: 3,
            }}
          >
            <Typography variant="h6" color="text.secondary" gutterBottom>
              No consoles yet
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              Create a console to start writing SQL queries.
            </Typography>
            <Button
              variant="contained"
              startIcon={<PlusIcon size={16} />}
              onClick={() => void handleCreateConsole()}
            >
              Create Console
            </Button>
          </Paper>
        </Box>
      ) : (
        <>
          <Box sx={{ borderBottom: 1, borderColor: 'divider', px: 2, display: 'flex', alignItems: 'center' }}>
            <Tabs
              value={activeConsoleId}
              onChange={handleTabChange}
              variant="scrollable"
              scrollButtons="auto"
              sx={{
                flex: 1,
                minHeight: 32,
                '& .MuiTabs-flexContainer': {
                  alignItems: 'center',
                },
                '& .MuiTab-root': {
                  textTransform: 'none',
                  minHeight: 32,
                  py: 0,
                  px: 1,
                },
              }}
            >
              {consoles.map((console) => (
                <Tab
                  key={console.id}
                  value={console.id}
                  label={
                    <Box sx={{ display: 'flex', alignItems: 'center' }}>
                      {editingConsoleId === console.id ? (
                        <input
                          type="text"
                          value={editingName}
                          onChange={(e) => setEditingName(e.target.value)}
                          onKeyDown={handleNameKeyDown}
                          onBlur={() => void handleSaveName()}
                          autoFocus
                          style={{
                            background: 'transparent',
                            border: 'none',
                            outline: 'none',
                            fontSize: 'inherit',
                            fontFamily: 'inherit',
                            color: 'inherit',
                            width: 120,
                            padding: 0,
                          }}
                        />
                      ) : (
                        <span
                          onDoubleClick={(e) => {
                            e.stopPropagation();
                            handleStartEditing(console.id, console.name);
                          }}
                          style={{ cursor: 'pointer' }}
                        >
                          {console.name || 'Untitled'}
                        </span>
                      )}
                      {consoles.length > 1 && (
                        <IconButton
                          size="small"
                          onClick={(e) => {
                            e.stopPropagation();
                            void handleDeleteConsole(console.id);
                          }}
                          sx={{ ml: 1.5, p: 0.25 }}
                        >
                          <CloseIcon size={12} />
                        </IconButton>
                      )}
                    </Box>
                  }
                />
              ))}
            </Tabs>
            <Button
              size="small"
              startIcon={<PlusIcon size={14} />}
              onClick={() => void handleCreateConsole()}
              sx={{ ml: 1, flexShrink: 0 }}
            >
              New
            </Button>
          </Box>

          <Box sx={{ flex: 1, p: 2, overflow: 'hidden' }}>
            {activeConsole && (
              loadingConsoleId === activeConsole.id ? (
                <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
                  <CircularProgress size={24} />
                </Box>
              ) : (
                <ConsoleEditor
                  key={activeConsole.id}
                  consoleId={activeConsole.id}
                  consoleName={activeConsole.name || 'Untitled'}
                  initialQuery={queryByConsole[activeConsole.id] ?? ''}
                  onQueryChange={(query) => handleQueryChange(activeConsole.id, query)}
                />
              )
            )}
          </Box>
        </>
      )}
    </Box>
  );
}
