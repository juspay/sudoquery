import { Typography } from '@mui/material';
import { colorBlue, colorInk, fontFamilyDisplay } from '../../theme/tokens';
import { Link } from 'react-router-dom';

export function Logo() {
  return (
    <Typography
      component={Link}
      to="/app/chat"
      sx={{
        fontFamily: fontFamilyDisplay,
        fontSize: '18px',
        fontWeight: 700,
        color: colorInk,
        letterSpacing: '-0.5px',
        textDecoration: 'none',
        display: 'flex',
        alignItems: 'center',
        flexShrink: 0,
        '& em': {
          color: colorBlue,
          fontStyle: 'italic',
        }
      }}
    >
      Sudo<em>query</em>
    </Typography>
  );
}
