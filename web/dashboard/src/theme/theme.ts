import { createTheme } from '@mui/material/styles';
import {
  colorCream,
  colorCream2,
  colorInk,
  colorInk60,
  colorBlue,
  colorBlueDark,
  colorRose,
  fontFamilyDisplay,
  fontFamilyBody,
  radiusButton,
  radiusCard,
  radiusInput,
} from './tokens';

export function createAppTheme() {
  return createTheme({
    palette: {
      mode: 'light',
      primary: {
        main: colorBlue,
        dark: colorBlueDark,
        contrastText: '#FFFFFF',
      },
      secondary: {
        main: colorInk60,
        contrastText: '#FFFFFF',
      },
      error: {
        main: colorRose,
      },
      background: {
        default: colorCream,
        paper: '#FFFFFF',
      },
      text: {
        primary: colorInk,
        secondary: colorInk60,
      },
      divider: colorCream2,
    },
    typography: {
      fontFamily: fontFamilyBody,
      fontSize: 14,
      h1: {
        fontFamily: fontFamilyDisplay,
        fontWeight: 700,
        fontSize: '2.5rem',
        lineHeight: 1.2,
        color: colorInk,
      },
      h2: {
        fontFamily: fontFamilyDisplay,
        fontWeight: 700,
        fontSize: '2rem',
        lineHeight: 1.25,
        color: colorInk,
      },
      h3: {
        fontFamily: fontFamilyDisplay,
        fontWeight: 600,
        fontSize: '1.5rem',
        lineHeight: 1.3,
        color: colorInk,
      },
      h4: {
        fontFamily: fontFamilyDisplay,
        fontWeight: 600,
        fontSize: '1.25rem',
        lineHeight: 1.35,
        color: colorInk,
      },
      h5: {
        fontFamily: fontFamilyBody,
        fontWeight: 600,
        fontSize: '1.1rem',
        color: colorInk,
      },
      h6: {
        fontFamily: fontFamilyBody,
        fontWeight: 600,
        fontSize: '1rem',
        color: colorInk,
      },
      body1: {
        fontFamily: fontFamilyBody,
        fontSize: '1.031rem', // ~16.5px (was 0.9375rem ~15px)
        fontWeight: 500,
        lineHeight: 1.6,
        color: colorInk,
      },
      body2: {
        fontFamily: fontFamilyBody,
        fontSize: '0.9625rem', // ~15.4px (was 0.875rem ~14px)
        fontWeight: 500,
        lineHeight: 1.5,
        color: colorInk60,
      },
      button: {
        fontFamily: fontFamilyBody,
        fontWeight: 600,
        letterSpacing: '0.01em',
        textTransform: 'none',
      },
      caption: {
        fontFamily: fontFamilyBody,
        fontSize: '0.75rem',
        fontWeight: 500,
        color: colorInk60,
      },
    },
    shape: {
      borderRadius: radiusCard,
    },
    components: {
      MuiCssBaseline: {
        styleOverrides: `
          html, body {
            height: 100%;
            margin: 0;
            background-color: ${colorCream};
            font-family: ${fontFamilyBody};
          }
          #root {
            height: 100%;
          }
          * {
            box-sizing: border-box;
          }
          ::-webkit-scrollbar {
            width: 6px;
            height: 6px;
          }
          ::-webkit-scrollbar-track {
            background: transparent;
          }
          ::-webkit-scrollbar-thumb {
            background: ${colorCream2};
            border-radius: 3px;
          }
          ::-webkit-scrollbar-thumb:hover {
            background: #d0ccc8;
          }
        `,
      },
      MuiButton: {
        styleOverrides: {
          root: {
            borderRadius: radiusButton,
            textTransform: 'none',
            fontWeight: 600,
            transition: 'all 0.18s ease',
            padding: '8px 20px',
          },
          contained: {
            boxShadow: 'none',
            '&:hover': {
              boxShadow: '0 2px 8px rgba(66,135,245,0.25)',
            },
          },
          outlined: {
            borderWidth: '1.5px',
            '&:hover': {
              borderWidth: '1.5px',
            },
          },
        },
        defaultProps: {
          disableElevation: true,
        },
      },
      MuiCard: {
        styleOverrides: {
          root: {
            borderRadius: radiusCard,
            boxShadow: '0 1px 3px rgba(0,0,0,0.06), 0 1px 2px rgba(0,0,0,0.04)',
            border: `1px solid ${colorCream2}`,
          },
        },
      },
      MuiPaper: {
        styleOverrides: {
          root: {
            backgroundImage: 'none',
          },
          elevation1: {
            boxShadow: '0 1px 3px rgba(0,0,0,0.06), 0 1px 2px rgba(0,0,0,0.04)',
          },
        },
      },
      MuiTextField: {
        styleOverrides: {
          root: {
            '& .MuiOutlinedInput-root': {
              borderRadius: radiusInput,
              '& fieldset': {
                borderColor: colorCream2,
                borderWidth: '1.5px',
              },
              '&:hover fieldset': {
                borderColor: colorInk60,
              },
              '&.Mui-focused fieldset': {
                borderColor: colorBlue,
                borderWidth: '2px',
              },
            },
          },
        },
      },
      MuiInputBase: {
        styleOverrides: {
          root: {
            borderRadius: `${radiusInput}px !important`,
          },
        },
      },
      MuiChip: {
        styleOverrides: {
          root: {
            borderRadius: 4,
            fontWeight: 500,
            transition: 'all 0.18s ease',
          },
        },
      },
      MuiIconButton: {
        styleOverrides: {
          root: {
            borderRadius: radiusButton,
            transition: 'all 0.18s ease',
          },
        },
      },
      MuiTooltip: {
        styleOverrides: {
          tooltip: {
            borderRadius: 4,
            fontSize: '0.75rem',
          },
        },
      },
      MuiListItemButton: {
        styleOverrides: {
          root: {
            borderRadius: radiusButton,
            transition: 'all 0.18s ease',
          },
        },
      },
      MuiAppBar: {
        styleOverrides: {
          root: {
            backgroundColor: '#FFFFFF',
            color: colorInk,
            boxShadow: `0 1px 0 ${colorCream2}`,
          },
        },
      },
      MuiDivider: {
        styleOverrides: {
          root: {
            borderColor: colorCream2,
          },
        },
      },
      MuiDialog: {
        styleOverrides: {
          paper: {
            borderRadius: 8,
          },
        },
      },
      MuiSvgIcon: {
        styleOverrides: {
          root: {
            strokeWidth: 1.2,
          },
        },
      },
    },
  });
}

const appTheme = createAppTheme();
