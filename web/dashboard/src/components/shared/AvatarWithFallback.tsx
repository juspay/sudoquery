import { Avatar, Tooltip } from '@mui/material';
import { colorBluePale, colorBlue } from '../../theme/tokens';

interface AvatarWithFallbackProps {
  photoUrl?: string | null;
  name?: string | null;
  email?: string | null;
  size?: number;
  showTooltip?: boolean;
}

function getInitials(name?: string | null, email?: string | null): string {
  if (name && name.trim()) {
    const parts = name.trim().split(' ');
    if (parts.length >= 2) {
      return `${parts[0][0]}${parts[parts.length - 1][0]}`.toUpperCase();
    }
    return name[0].toUpperCase();
  }
  if (email) {
    return email[0].toUpperCase();
  }
  return '?';
}

export function AvatarWithFallback({
  photoUrl,
  name,
  email,
  size = 28,
  showTooltip = false,
}: AvatarWithFallbackProps) {
  const initials = getInitials(name, email);
  const displayName = name || email || 'User';

  const avatar = (
    <Avatar
      src={photoUrl ?? undefined}
      alt={displayName}
      sx={{
        width: size,
        height: size,
        bgcolor: colorBluePale,
        color: colorBlue,
        fontSize: '12px',
        fontWeight: 600,
        border: `2px solid rgba(24,22,15,0.09)`,
        cursor: 'pointer',
        transition: 'border-color 0.15s',
        '&:hover': {
          borderColor: colorBlue,
        }
      }}
    >
      {!photoUrl && initials}
    </Avatar>
  );

  if (showTooltip) {
    return (
      <Tooltip title={displayName} placement="bottom">
        {avatar}
      </Tooltip>
    );
  }

  return avatar;
}
