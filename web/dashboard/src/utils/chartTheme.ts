/**
 * Theme-aware colors for Nivo charts
 * Dynamically detects current theme (light/dark) and returns appropriate colors
 */

function isDarkMode(): boolean {
  if (typeof document === 'undefined') return false;
  return document.documentElement.classList.contains('dark');
}

/**
 * Get theme-aware text color
 */
export function getTextColor(): string {
  return isDarkMode() ? '#e0e0e0' : '#333333';
}

/**
 * Get theme-muted text color
 */
export function getMutedTextColor(): string {
  return isDarkMode() ? '#999999' : '#666666';
}

/**
 * Get theme-aware axis color
 */
export function getAxisColor(): string {
  return isDarkMode() ? '#757575' : '#e0e0e0';
}

/**
 * Get theme-aware grid line color
 */
function getGridColor(): string {
  return isDarkMode() ? '#424242' : '#eeeeee';
}

/**
 * Get theme-aware tooltip background
 */
export function getTooltipBackground(): string {
  return isDarkMode() ? '#424242' : '#ffffff';
}

/**
 * Get theme-aware tooltip text
 */
export function getTooltipTextColor(): string {
  return isDarkMode() ? '#ffffff' : '#333333';
}