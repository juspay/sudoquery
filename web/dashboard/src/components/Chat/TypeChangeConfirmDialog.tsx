import {
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Button,
} from '@mui/material';
import type { ChatType } from '../../types/chat';
import { ChatTypeLabels } from '../../types/chat';

interface TypeChangeConfirmDialogProps {
  isOpen: boolean;
  fromType: ChatType;
  toType: ChatType;
  onConfirm: (chatType: ChatType) => void;
  onCancel: () => void;
}

export function TypeChangeConfirmDialog({
  isOpen,
  fromType,
  toType,
  onConfirm,
  onCancel,
}: TypeChangeConfirmDialogProps) {
  return (
    <Dialog open={isOpen} onClose={onCancel}>
      <DialogTitle>Create New Chat?</DialogTitle>
      <DialogContent>
        <DialogContentText>
          Switching from {ChatTypeLabels[fromType]} to {ChatTypeLabels[toType]} requires creating a new chat.
          Your current conversation will remain unchanged.
        </DialogContentText>
      </DialogContent>
      <DialogActions>
        <Button onClick={onCancel}>Cancel</Button>
        <Button onClick={()=>onConfirm(toType)} variant="contained" color="primary">
          Create New Chat
        </Button>
      </DialogActions>
    </Dialog>
  );
}
