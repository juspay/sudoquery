import { useState } from 'react';
import {
  Box,
  Typography,
  Popover,
  TextField,
  InputAdornment,
  Button,
  IconButton,
  Tooltip,
} from '@mui/material';
import {
  Folder as FolderIcon,
  Search as SearchIcon,
  Plus as AddIcon,
  CheckCircle as CheckCircleIcon,
  ChevronRight as ChevronRightIcon,
  Copy as CopyIcon,
  Check as CheckIcon,
} from 'lucide-react';
import { useOrganization } from '../../contexts/OrganizationContext';
import { useProject } from '../../contexts/ProjectContext';
import { useToast } from '../../contexts/ToastContext';
import {
  colorInk,
  colorCream2,
  colorCream3,
  colorBluePale,
  colorInk60,
  colorInk40,
  fontFamilyBody,
  fontFamilyDisplay,
  fontFamilyMono,
} from '../../theme/tokens';
import { useNavigate } from 'react-router-dom';
import { CreateOrganizationDialog } from '../Organization/CreateOrganizationDialog';
import { CreateProjectDialog } from '../Organization/CreateProjectDialog';

// Get initials from name (max 2 characters)
function getInitials(name: string): string {
  const words = name.trim().split(/\s+/);
  if (words.length === 1) {
    return words[0].slice(0, 2).toUpperCase();
  }
  return (words[0][0] + words[words.length - 1][0]).toUpperCase();
}

// Get a consistent color for an org based on name
function getOrgColor(name: string): string {
  const colors = ['#A5402C', '#4287f5', '#2AA99E', '#6B7280', '#8B5A2B', '#4A5568'];
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
}

// The copy button shows while its row is hovered, while it has keyboard focus,
// while it says "Copied", and always on touch screens, which can't hover.
const copyIdButtonReveal = {
  '& .copy-id': { opacity: 0, transition: 'opacity 0.15s' },
  '&:hover .copy-id, & .copy-id:focus-visible, & .copy-id.copied': { opacity: 1 },
  '@media (hover: none)': { '& .copy-id': { opacity: 1 } },
};

interface CopyIdButtonProps {
  id: string;
  /** What the ID is called, e.g. "tenant ID". */
  label: string;
}

function CopyIdButton({ id, label }: CopyIdButtonProps) {
  const { showError } = useToast();
  const [copied, setCopied] = useState(false);

  const copy = async (e: React.MouseEvent) => {
    // Copying shouldn't also select the row.
    e.stopPropagation();
    try {
      await navigator.clipboard.writeText(id);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // There's no clipboard outside secure contexts (e.g. plain http), so
      // show the ID for copying by hand.
      showError(`Couldn't copy the ${label}: ${id}`);
    }
  };

  return (
    <Tooltip title={copied ? 'Copied' : `Copy ${label}`} placement="top">
      <IconButton
        className={copied ? 'copy-id copied' : 'copy-id'}
        size="small"
        aria-label={`Copy ${label}`}
        onClick={copy}
        sx={{ flexShrink: 0, color: copied ? '#2AA99E' : colorInk40, '&:hover': { color: colorInk } }}
      >
        {copied ? <CheckIcon size={16} /> : <CopyIcon size={16} />}
      </IconButton>
    </Tooltip>
  );
}

interface OrgItemProps {
  id: string;
  name: string;
  selected: boolean;
  onClick: () => void;
}

function OrgItem({ id, name, selected, onClick }: OrgItemProps) {
  const initials = getInitials(name);
  const bgColor = getOrgColor(name);

  return (
    <Box
      onClick={onClick}
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: 1,
        py: 0.75,
        px: 1,
        borderRadius: 1,
        cursor: 'pointer',
        bgcolor: selected ? colorCream2 : 'transparent',
        '&:hover': { bgcolor: selected ? colorCream2 : 'rgba(0,0,0,0.02)' },
        ...copyIdButtonReveal,
      }}
    >
      <Box
        sx={{
          width: 28,
          height: 28,
          borderRadius: 0.75,
          bgcolor: bgColor,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          color: 'white',
          fontSize: '11px',
          fontWeight: 600,
          fontFamily: fontFamilyBody,
          flexShrink: 0,
          lineHeight: 1,
        }}
      >
        {initials}
      </Box>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography
          sx={{
            fontSize: '13px',
            fontWeight: 500,
            color: colorInk,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {name}
        </Typography>
      </Box>
      <CopyIdButton id={id} label="tenant ID" />
      {selected && (
        <CheckCircleIcon size={20} style={{ color: '#2AA99E', flexShrink: 0 }} />
      )}
    </Box>
  );
}

interface ProjectItemProps {
  id: string;
  name: string;
  selected: boolean;
  onClick: () => void;
}

function ProjectItem({ id, name, selected, onClick }: ProjectItemProps) {
  return (
    <Box
      onClick={onClick}
      sx={{
        display: 'flex',
        alignItems: 'center',
        gap: 1,
        py: 0.75,
        px: 1,
        borderRadius: 1,
        cursor: 'pointer',
        bgcolor: selected ? colorCream2 : 'transparent',
        '&:hover': { bgcolor: selected ? colorCream2 : 'rgba(0,0,0,0.02)' },
        ...copyIdButtonReveal,
      }}
    >
      <Box
        sx={{
          width: 28,
          height: 28,
          borderRadius: 0.75,
          bgcolor: colorCream3,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          flexShrink: 0,
        }}
      >
        <FolderIcon size={20} style={{ color: colorInk60 }} />
      </Box>
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Typography
          sx={{
            fontSize: '13px',
            fontWeight: 500,
            color: colorInk,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {name}
        </Typography>
      </Box>
      <CopyIdButton id={id} label="workspace ID" />
      {selected && (
        <CheckCircleIcon size={20} style={{ color: '#2AA99E', flexShrink: 0 }} />
      )}
    </Box>
  );
}

export function OrgProjectSwitcher() {
  const {
    organizations,
    currentOrganization,
    setCurrentOrganization,
  } = useOrganization();
  const { projects, currentProject, setCurrentProject } = useProject();
  const navigate = useNavigate();

  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const open = Boolean(anchorEl);

  const [orgSearch, setOrgSearch] = useState('');
  const [projectSearch, setProjectSearch] = useState('');

  const [createOrgDialogOpen, setCreateOrgDialogOpen] = useState(false);
  const [createProjectDialogOpen, setCreateProjectDialogOpen] = useState(false);

  const handleClick = (e: React.MouseEvent<HTMLElement>) => setAnchorEl(e.currentTarget);
  const handleClose = () => setAnchorEl(null);

  const handleSelectOrg = (orgId: string) => {
    if (orgId !== currentOrganization?.id) {
      const org = organizations.find((o) => o.id === orgId) || null;
      setCurrentOrganization(org);
    }
  };

  const handleSelectProject = (projectId: string) => {
    const proj = projects.find((p) => p.id === projectId) || null;
    setCurrentProject(proj);
    handleClose();
  };

  const filteredOrgs = organizations.filter((org) =>
    org.name.toLowerCase().includes(orgSearch.toLowerCase())
  );
  const filteredProjects = projects.filter((proj) =>
    proj.name.toLowerCase().includes(projectSearch.toLowerCase())
  );

  if (!currentOrganization) return null;

  const orgInitials = getInitials(currentOrganization.name);
  const orgColor = getOrgColor(currentOrganization.name);

  return (
    <>
      {/* Trigger Button */}
      <Box
        onClick={handleClick}
        sx={{
          display: 'flex',
          alignItems: 'center',
          gap: { xs: 0, md: 1 },
          cursor: 'pointer',
          px: { xs: '8px', md: '12px' },
          height: 40,
          bgcolor: 'white',
          border: '1px solid rgba(24,22,15,0.12)',
          borderRadius: '6px',
          fontFamily: fontFamilyBody,
          fontSize: '14px',
          fontWeight: 500,
          color: colorInk,
          transition: 'all 0.15s',
          '&:hover': { borderColor: 'rgba(24,22,15,0.2)', boxShadow: '0 1px 3px rgba(0,0,0,0.04)' },
        }}
        role="button"
        aria-haspopup="true"
        aria-expanded={open ? 'true' : undefined}
      >
        <Box
          sx={{
            width: 28,
            height: 28,
            borderRadius: 1,
            bgcolor: orgColor,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            color: 'white',
            fontSize: '11px',
            fontWeight: 600,
            fontFamily: fontFamilyBody,
            flexShrink: 0,
          }}
        >
          {orgInitials}
        </Box>
        <Box sx={{ display: { xs: 'none', md: 'block' }, maxWidth: 140, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {currentOrganization.name}
        </Box>
        <ChevronRightIcon size={18} style={{ color: colorInk40 }} sx={{ display: { xs: 'none', md: 'block' }, mx: 0.25 }} />
        <FolderIcon size={20} style={{ color: colorInk60 }} sx={{ display: { xs: 'none', md: 'block' } }} />
        <Box sx={{ display: { xs: 'none', md: 'block' }, maxWidth: 140, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {currentProject?.name || 'Select Project'}
        </Box>
      </Box>

      {/* Dropdown Popover */}
      <Popover
        open={open}
        anchorEl={anchorEl}
        onClose={handleClose}
        anchorOrigin={{ vertical: 'bottom', horizontal: 'center' }}
        transformOrigin={{ vertical: 'top', horizontal: 'center' }}
        PaperProps={{
          elevation: 0,
          sx: {
            mt: 1,
            width: { xs: 'calc(100vw - 32px)', md: 600 },
            maxWidth: '90vw',
            border: `1px solid ${colorCream3}`,
            boxShadow: '0 4px 20px rgba(0,0,0,0.08)',
            borderRadius: 2,
            overflow: 'hidden',
          },
        }}
      >
        <Box sx={{ display: 'flex', height: 400 }}>
          {/* Organization Column */}
          <Box sx={{ flex: 1, display: 'flex', flexDirection: 'column', borderRight: `1px solid ${colorCream3}` }}>
            <Box sx={{ px: 2, py: 1.5 }}>
              <Typography sx={{ fontSize: '11px', fontWeight: 600, letterSpacing: '0.08em', color: colorInk60, textTransform: 'uppercase' }}>
                Organization
              </Typography>
            </Box>
            <Box sx={{ px: 2, pb: 1.5 }}>
              <TextField
                fullWidth
                placeholder="Search organization..."
                value={orgSearch}
                onChange={(e) => setOrgSearch(e.target.value)}
                size="small"
                InputProps={{
                  startAdornment: (
                    <InputAdornment position="start">
                      <SearchIcon size={18} style={{ color: colorInk40 }} />
                    </InputAdornment>
                  ),
                }}
                sx={{
                  '& .MuiOutlinedInput-root': {
                    borderRadius: 1.5,
                    bgcolor: 'white',
                    '& fieldset': { borderColor: colorCream3 },
                    '&:hover fieldset': { borderColor: colorInk40 },
                    '&.Mui-focused fieldset': { borderColor: colorInk60 },
                  },
                }}
              />
            </Box>
            <Box sx={{ flex: 1, overflow: 'auto', px: 1 }}>
              {filteredOrgs.map((org) => (
                <OrgItem
                  key={org.id}
                  id={org.id}
                  name={org.name}
                  selected={org.id === currentOrganization?.id}
                  onClick={() => handleSelectOrg(org.id)}
                />
              ))}
            </Box>
            <Box sx={{ p: 1.5, borderTop: `1px solid ${colorCream3}` }}>
              <Button
                fullWidth
                onClick={() => {
                  handleClose();
                  setCreateOrgDialogOpen(true);
                }}
                startIcon={<AddIcon />}
                sx={{
                  justifyContent: 'flex-start',
                  color: colorInk60,
                  textTransform: 'none',
                  fontSize: '14px',
                  fontWeight: 500,
                  py: 1,
                  borderRadius: 1,
                  '&:hover': { bgcolor: colorCream2 },
                }}
              >
                New organization
              </Button>
            </Box>
          </Box>

          {/* Project Column */}
          <Box sx={{ flex: 1, display: 'flex', flexDirection: 'column' }}>
            <Box sx={{ px: 2, py: 1.5 }}>
              <Typography sx={{ fontSize: '11px', fontWeight: 600, letterSpacing: '0.08em', color: colorInk60, textTransform: 'uppercase' }}>
                Project
              </Typography>
            </Box>
            <Box sx={{ px: 2, pb: 1.5 }}>
              <TextField
                fullWidth
                placeholder="Search project..."
                value={projectSearch}
                onChange={(e) => setProjectSearch(e.target.value)}
                size="small"
                InputProps={{
                  startAdornment: (
                    <InputAdornment position="start">
                      <SearchIcon size={18} style={{ color: colorInk40 }} />
                    </InputAdornment>
                  ),
                }}
                sx={{
                  '& .MuiOutlinedInput-root': {
                    borderRadius: 1.5,
                    bgcolor: 'white',
                    '& fieldset': { borderColor: colorCream3 },
                    '&:hover fieldset': { borderColor: colorInk40 },
                    '&.Mui-focused fieldset': { borderColor: colorInk60 },
                  },
                }}
              />
            </Box>
            <Box sx={{ flex: 1, overflow: 'auto', px: 1 }}>
              {filteredProjects.length === 0 ? (
                <Box sx={{ p: 2, textAlign: 'center' }}>
                  <Typography sx={{ fontSize: '14px', color: colorInk40 }}>No projects found</Typography>
                </Box>
              ) : (
                filteredProjects.map((proj) => (
                  <ProjectItem
                    key={proj.id}
                    id={proj.id}
                    name={proj.name}
                    selected={proj.id === currentProject?.id}
                    onClick={() => handleSelectProject(proj.id)}
                  />
                ))
              )}
            </Box>
            <Box sx={{ p: 1.5, borderTop: `1px solid ${colorCream3}` }}>
              <Button
                fullWidth
                onClick={() => {
                  handleClose();
                  setCreateProjectDialogOpen(true);
                }}
                startIcon={<AddIcon />}
                sx={{
                  justifyContent: 'flex-start',
                  color: colorInk60,
                  textTransform: 'none',
                  fontSize: '14px',
                  fontWeight: 500,
                  py: 1,
                  borderRadius: 1,
                  '&:hover': { bgcolor: colorCream2 },
                }}
              >
                New project
              </Button>
            </Box>
          </Box>
        </Box>
      </Popover>

      {/* Dialogs */}
      <CreateOrganizationDialog
        open={createOrgDialogOpen}
        onClose={() => setCreateOrgDialogOpen(false)}
        onSuccess={(org) => {
          setCreateOrgDialogOpen(false);
          setCurrentOrganization(org);
        }}
      />
      <CreateProjectDialog
        open={createProjectDialogOpen}
        onClose={() => setCreateProjectDialogOpen(false)}
        onSuccess={(project) => {
          setCreateProjectDialogOpen(false);
          setCurrentProject(project);
        }}
      />
    </>
  );
}
