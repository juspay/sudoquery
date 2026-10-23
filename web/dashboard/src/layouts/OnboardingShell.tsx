import { useState } from 'react';
import { Outlet } from 'react-router-dom';
import { Box, Button, Container, ListItemIcon, Menu, MenuItem } from '@mui/material';
import { ChevronDown, LogOut } from 'lucide-react';
import { useAuth } from '../contexts/AuthContext';
import { colorCream, colorInk } from '../theme/tokens';

export default function OnboardingShell() {
  const { user, logout } = useAuth();
  const identity = user?.email || user?.preferred_username;
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);

  return (
    <Box
      sx={{
        minHeight: '100vh',
        bgcolor: colorCream,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        overflow: 'auto',
        position: 'relative',
      }}
    >
      {identity ? (
        <Box sx={{ position: 'absolute', top: 12, right: 16 }}>
          <Button
            size="small"
            onClick={(e) => setAnchor(e.currentTarget)}
            endIcon={<ChevronDown size={14} />}
            aria-haspopup="menu"
            aria-expanded={anchor ? 'true' : undefined}
            sx={{ textTransform: 'none', color: colorInk, fontSize: '13px', fontWeight: 600 }}
          >
            {identity}
          </Button>
          <Menu anchorEl={anchor} open={Boolean(anchor)} onClose={() => setAnchor(null)}>
            <MenuItem
              onClick={() => {
                setAnchor(null);
                logout();
              }}
            >
              <ListItemIcon>
                <LogOut size={16} />
              </ListItemIcon>
              Log out
            </MenuItem>
          </Menu>
        </Box>
      ) : null}
      <Container maxWidth="sm" sx={{ py: 6 }}>
        <Outlet />
      </Container>
    </Box>
  );
}
