import { useState } from 'react';
import { Box, Typography, Table, TableBody, TableCell, TableContainer, TableHead, TableRow, IconButton, Menu, MenuItem } from '@mui/material';
import { MoreVertical as MoreIcon } from 'lucide-react';
import { RoleBadge } from './RoleBadge';
import type { RoleType } from './RoleBadge';
import { AvatarWithFallback } from '../shared/AvatarWithFallback';
import { colorCream2 } from '../../theme/tokens';

export interface MemberRow {
  id: string;
  name: string;
  email: string;
  role: RoleType;
  joinedAt: Date;
}

interface RoleOption {
  value: RoleType;
  label: string;
}

interface MembersTableProps {
  members: MemberRow[];
  onUpdateRole?: (id: string, newRole: RoleType) => void;
  onRemoveMember?: (id: string) => void;
  isAdmin?: boolean;
  roleOptions?: RoleOption[];
  removeLabel?: string;
}

export function MembersTable({ members, onUpdateRole, onRemoveMember, isAdmin = false, roleOptions, removeLabel = 'Remove from Organization' }: MembersTableProps) {
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const [selectedMember, setSelectedMember] = useState<MemberRow | null>(null);

  const defaultRoleOptions: RoleOption[] = [
    { value: 'org_admin', label: 'Org Admin' },
    { value: 'org_user', label: 'Org Member' },
  ];
  const effectiveRoleOptions = roleOptions ?? defaultRoleOptions;

  const handleMenuClick = (event: React.MouseEvent<HTMLElement>, member: MemberRow) => {
    setAnchorEl(event.currentTarget);
    setSelectedMember(member);
  };

  const handleClose = () => {
    setAnchorEl(null);
    setSelectedMember(null);
  };

  const handleRoleChange = (role: RoleType) => {
    if (selectedMember && onUpdateRole) {
      onUpdateRole(selectedMember.id, role);
    }
    handleClose();
  };

  const handleRemove = () => {
    if (selectedMember && onRemoveMember) {
      onRemoveMember(selectedMember.id);
    }
    handleClose();
  };

  return (
    <Box sx={{ border: `1px solid ${colorCream2}`, borderRadius: 2, overflow: 'hidden', bgcolor: '#fff' }}>
      <TableContainer>
        <Table sx={{ minWidth: 600 }} aria-label="members table">
          <TableHead sx={{ bgcolor: 'rgba(28,25,23,0.02)' }}>
            <TableRow>
              <TableCell>User</TableCell>
              <TableCell>Role</TableCell>
              <TableCell>Joined</TableCell>
              {isAdmin && <TableCell align="right">Actions</TableCell>}
            </TableRow>
          </TableHead>
          <TableBody>
            {members.map((member) => (
              <TableRow key={member.id} sx={{ '&:last-child td, &:last-child th': { border: 0 } }}>
                <TableCell component="th" scope="row">
                  <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
                    <AvatarWithFallback name={member.name} email={member.email} size={36} />
                    <Box>
                      <Typography variant="body2" fontWeight={600}>{member.name}</Typography>
                      <Typography variant="caption" color="text.secondary">{member.email}</Typography>
                    </Box>
                  </Box>
                </TableCell>
                <TableCell>
                  <RoleBadge role={member.role} />
                </TableCell>
                <TableCell>
                  <Typography variant="body2" color="text.secondary">
                    {new Intl.DateTimeFormat('en-US', { month: 'short', day: 'numeric', year: 'numeric' }).format(member.joinedAt)}
                  </Typography>
                </TableCell>
                {isAdmin && (
                  <TableCell align="right">
                    <IconButton size="small" onClick={(e) => handleMenuClick(e, member)}>
                      <MoreIcon size={16} />
                    </IconButton>
                  </TableCell>
                )}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </TableContainer>

      {isAdmin && (
        <Menu
          anchorEl={anchorEl}
          open={Boolean(anchorEl)}
          onClose={handleClose}
          elevation={2}
          PaperProps={{ sx: { minWidth: 160, borderRadius: 2, mt: 0.5 } }}
        >
          <MenuItem disabled sx={{ opacity: '1 !important', py: 0.5, mb: 1 }}>
            <Typography variant="caption" fontWeight={600} color="text.secondary">
              CHANGE ROLE
            </Typography>
          </MenuItem>
          {effectiveRoleOptions
            .filter((option) => option.value !== selectedMember?.role)
            .map((option, index, arr) => (
              <MenuItem
                key={option.value}
                onClick={() => handleRoleChange(option.value)}
                sx={index === arr.length - 1 ? { mb: 1 } : {}}
              >
                {option.label}
              </MenuItem>
            ))}
          <MenuItem onClick={handleRemove} sx={{ color: 'error.main' }}>
            {removeLabel}
          </MenuItem>
        </Menu>
      )}
    </Box>
  );
}
