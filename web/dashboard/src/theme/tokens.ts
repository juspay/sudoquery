// ─── Sudoquery Design Tokens ────────────────────────────────────────────────
// "Warm Editorial Bold" design system

// ─── Color Palette ────────────────────────────────────────────────────────────

/** Main background — warm off-white */
export const colorCream = '#F8F6F0';
/** Slightly warmer cream for cards/surfaces */
export const colorCream2 = '#F0EDE4';
/** Subtle border / divider color */
export const colorCream3 = '#E8E4DA';
const colorCream4 = '#DDD8CC';

/** Primary text — rich dark ink */
export const colorInk = '#18160F';
export const colorInk80 = '#3A3830';
/** Secondary / muted text */
export const colorInk60 = '#6A6760';
/** Placeholder / disabled text */
export const colorInk40 = '#928F88';
/** Very subtle overlay */
export const colorInk20 = '#C0BDB6';

/** Primary action — teal */
export const colorBlue = '#4287f5';
/** Hover / darker teal */
export const colorBlueDark = '#1d4ed8';
const colorBlue2 = '#2563eb';
const colorBlue3 = '#60a5fa';
/** Pale teal — active nav bg, chip bg */
export const colorBluePale = '#dbeafe';
/** Light teal text on pale bg */
export const colorBlueLight = '#2AA99E';

/** Danger / destructive — rose */
export const colorRose = '#A5402C';
/** Darker rose for hover */
export const colorRoseDark = '#8B3625';
/** Pale rose — danger zone border bg */
export const colorRosePale = '#F5E2DC';

// ─── Typography ───────────────────────────────────────────────────────────────

export const fontFamilyDisplay = "'Playfair Display', serif";
export const fontFamilyBody = "'DM Sans', sans-serif";
export const fontFamilyMono = "'Source Code Pro', monospace";

// ─── Border Radii ─────────────────────────────────────────────────────────────

export const radiusButton = 3;       // px — confident, editorial feel
export const radiusCard = 4;         // px — contained card
export const radiusInput = 5;        // px — form fields
const radiusChartBlock = 6;   // px — inline chart cards
/** Asymmetric chat bubble radii: [TL, TR, BR, BL] */
const radiusBubbleAI = [2, 10, 10, 10];   // AI: flat top-left
const radiusBubbleUser = [10, 10, 2, 10]; // User: flat bottom-right
const radiusAvatar = '50%';

// ─── Navigation / Shell ───────────────────────────────────────────────────────

export const topbarHeight = 58;          // px
const sidenavCollapsedWidth = 44; // px
const sidenavExpandedWidth = 220; // px

// ─── Transitions ──────────────────────────────────────────────────────────────

const transitionDefault = 'all 0.18s ease';

const chatBubbleRadius = 16;
