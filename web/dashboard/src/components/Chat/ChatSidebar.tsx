import { Box, TextField, InputAdornment, Dialog, DialogTitle, DialogContent, DialogActions, Button, Typography } from '@mui/material';
import { useState } from 'react';
import { Search as SearchIcon, Plus as AddIcon, Trash2 as DeleteIcon } from 'lucide-react';
import type { Chat } from '../../types/chat';
import {
  colorCream,
  colorCream2,
  colorInk,
  colorInk40,
  colorInk60,
  colorBlue,
  colorBluePale,
} from '../../theme/tokens';

interface ChatSidebarProps {
  chats: Chat[];
  activeChatId: string | null;
  onSelectChat: (id: string) => void;
  onNewChat: () => void;
  onDeleteChat: (id: string) => void;
}

// Separate component for each chat item to properly use hooks
function ChatItem({
  chat,
  isActive,
  onSelect,
  onDelete,
}: {
  chat: Chat;
  isActive: boolean;
  onSelect: (id: string) => void;
  onDelete: (e: React.MouseEvent, id: string) => void;
}) {
  const [isHovered, setIsHovered] = useState(false);

  return (
    <Box
      onClick={() => onSelect(chat.id)}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      sx={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: '8px',
        p: '7px 8px',
        borderRadius: '7px',
        cursor: 'pointer',
        transition: 'background 0.12s',
        backgroundColor: isActive ? colorCream : 'transparent',
        border: isActive ? '1px solid rgba(24,22,15,0.15)' : '1px solid transparent',
        '&:hover': {
          backgroundColor: colorCream,
        },
      }}
    >
      <Box
        sx={{
          flex: 1,
          minWidth: 0,
          fontSize: '13px',
          color: isActive ? colorInk : colorInk60,
          whiteSpace: 'nowrap',
          overflow: 'hidden',
          textOverflow: 'ellipsis',
          lineHeight: 1.4,
          fontWeight: isActive ? 500 : 400,
        }}
      >
        {chat.title}
      </Box>
      <Box
        onClick={(e) => onDelete(e, chat.id)}
        sx={{
          opacity: isHovered ? 0.5 : 0,
          transition: 'all 0.15s',
          flexShrink: 0,
          cursor: 'pointer',
          color: colorInk60,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          p: '2px',
          borderRadius: '4px',
          '&:hover': {
            opacity: '1 !important',
            color: '#A32D2D',
            bgcolor: 'rgba(163, 45, 45, 0.08)',
          },
        }}
      >
        <DeleteIcon size={14} />
      </Box>
    </Box>
  );
}

export function ChatSidebar({
  chats,
  activeChatId,
  onSelectChat,
  onNewChat,
  onDeleteChat,
}: ChatSidebarProps) {
  const [filter, setFilter] = useState('');
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [chatToDelete, setChatToDelete] = useState<string | null>(null);

  const filteredChats = chats.filter(
    (c) => !filter || c.title.toLowerCase().includes(filter.toLowerCase())
  );

  const handleDeleteClick = (e: React.MouseEvent, id: string) => {
    e.stopPropagation();
    setChatToDelete(id);
    setDeleteDialogOpen(true);
  };

  const handleConfirmDelete = () => {
    if (chatToDelete) {
      onDeleteChat(chatToDelete);
    }
    setDeleteDialogOpen(false);
    setChatToDelete(null);
  };

  const handleCancelDelete = () => {
    setDeleteDialogOpen(false);
    setChatToDelete(null);
  };

  return (
    <Box
      sx={{
        width: 220,
        flexShrink: 0,
        height: '100%',
        background: colorCream2,
        borderRight: '1px solid rgba(24,22,15,0.09)',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      <Box
        sx={{
          p: '12px 10px 10px',
          borderBottom: '1px solid rgba(24,22,15,0.09)',
          display: 'flex',
          flexDirection: 'column',
          gap: '8px',
          flexShrink: 0,
        }}
      >
        <Box
          onClick={onNewChat}
          sx={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            p: '7px 10px',
            background: colorCream,
            border: '1px solid rgba(24,22,15,0.15)',
            borderRadius: '6px',
            fontSize: '12px',
            fontWeight: 500,
            color: colorInk,
            cursor: 'pointer',
            transition: 'border-color 0.15s, background 0.15s',
            whiteSpace: 'nowrap',
            '&:hover': {
              borderColor: colorBlue,
              background: colorBluePale,
            },
          }}
        >
          <span>New chat</span>
          <AddIcon size={13} style={{ color: colorInk60 }} />
        </Box>

        <TextField
          placeholder="Search chats…"
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          size="small"
          fullWidth
          InputProps={{
            startAdornment: (
              <InputAdornment position="start">
                <SearchIcon size={12} style={{ color: colorInk40 }} />
              </InputAdornment>
            ),
            sx: {
              fontSize: '12px',
              background: colorCream,
              '& fieldset': {
                borderColor: 'rgba(24,22,15,0.15)',
                borderRadius: '6px',
              },
            },
          }}
          sx={{
            '& .MuiInputBase-root': {
              height: 28,
              pl: '8px',
            },
            '& .MuiInputBase-input': {
              p: '6px 8px',
              fontSize: '12px',
              '&::placeholder': {
                color: colorInk40,
                opacity: 1,
              },
            },
          }}
        />
      </Box>

      <Box sx={{ fontSize: '11px', fontWeight: 600, letterSpacing: '0.06em', textTransform: 'uppercase', color: colorInk40, px: '10px', pt: '8px' }}>
        Chats
      </Box>

      <Box
        sx={{
          flex: 1,
          overflowY: 'auto',
          p: '4px 8px 12px',
          '&::-webkit-scrollbar': { width: '3px' },
          '&::-webkit-scrollbar-thumb': {
            background: 'rgba(24,22,15,0.15)',
            borderRadius: '2px',
          },
        }}
      >
        {filteredChats.map((c) => (
          <ChatItem
            key={c.id}
            chat={c}
            isActive={c.id === activeChatId}
            onSelect={onSelectChat}
            onDelete={handleDeleteClick}
          />
        ))}

        {filteredChats.length === 0 && (
          <Box
            sx={{
              p: '16px 8px',
              fontSize: '12px',
              color: colorInk40,
              textAlign: 'center',
            }}
          >
            No chats found
          </Box>
        )}
      </Box>

      {/* Delete Confirmation Dialog */}
      <Dialog open={deleteDialogOpen} onClose={handleCancelDelete}>
        <DialogTitle>Delete Chat?</DialogTitle>
        <DialogContent>
          <Typography variant="body2">
            Are you sure you want to delete this chat? This action cannot be undone.
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={handleCancelDelete}>Cancel</Button>
          <Button onClick={handleConfirmDelete} color="error" variant="contained">
            Delete
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
