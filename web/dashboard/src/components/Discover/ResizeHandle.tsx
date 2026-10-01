import { useRef, useState } from 'react';
import { Box } from '@mui/material';
import { colorBlue, colorInk20 } from '../../theme/tokens';

/** Pixels moved per arrow-key press. */
const KEY_STEP = 16;

interface ResizeHandleProps {
  /**
   * `column` sits between side-by-side panels and drags left and right;
   * `row` sits between stacked panels and drags up and down.
   */
  direction: 'column' | 'row';
  /** Size of the panel before the handle (left or above), in pixels. */
  size: number;
  min: number;
  max: number;
  onResize: (size: number) => void;
  /** Double-click puts the default size back. */
  onReset: () => void;
  label: string;
}

/**
 * The gap between two panels, which can be dragged — or moved with the arrow
 * keys once focused — to resize the panel before it.
 */
export function ResizeHandle({ direction, size, min, max, onResize, onReset, label }: ResizeHandleProps) {
  const drag = useRef<{ start: number; size: number } | null>(null);
  const [dragging, setDragging] = useState(false);
  const horizontal = direction === 'column';
  const cursor = horizontal ? 'col-resize' : 'row-resize';

  const position = (event: React.PointerEvent) => (horizontal ? event.clientX : event.clientY);

  const stop = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    drag.current = null;
    setDragging(false);
    event.currentTarget.releasePointerCapture(event.pointerId);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  };

  const handleKeyDown = (event: React.KeyboardEvent) => {
    const keys: Record<string, number> = horizontal
      ? { ArrowLeft: -KEY_STEP, ArrowRight: KEY_STEP }
      : { ArrowUp: -KEY_STEP, ArrowDown: KEY_STEP };
    if (event.key in keys) {
      event.preventDefault();
      onResize(size + keys[event.key]);
    } else if (event.key === 'Home') {
      event.preventDefault();
      onResize(min);
    } else if (event.key === 'End') {
      event.preventDefault();
      onResize(max);
    }
  };

  return (
    <Box
      role="separator"
      aria-label={label}
      aria-orientation={horizontal ? 'vertical' : 'horizontal'}
      aria-valuenow={size}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      title={`${label} — drag to resize, double-click to reset`}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture(event.pointerId);
        drag.current = { start: position(event), size };
        setDragging(true);
        // Keep the resize cursor and stop text selection while the pointer
        // wanders over the panels.
        document.body.style.cursor = cursor;
        document.body.style.userSelect = 'none';
      }}
      onPointerMove={(event) => {
        if (drag.current) {
          onResize(drag.current.size + position(event) - drag.current.start);
        }
      }}
      onPointerUp={stop}
      onPointerCancel={stop}
      onDoubleClick={onReset}
      onKeyDown={handleKeyDown}
      sx={{
        flexShrink: 0,
        alignSelf: 'stretch',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor,
        touchAction: 'none',
        outline: 'none',
        ...(horizontal ? { width: 12 } : { height: 12 }),
        // The visible line: faint at rest, blue while hovered, dragged or focused.
        '&::after': {
          content: '""',
          borderRadius: 1,
          bgcolor: dragging ? colorBlue : 'transparent',
          transition: 'background-color 0.12s',
          ...(horizontal ? { width: 2, height: '100%' } : { height: 2, width: '100%' }),
        },
        '&:hover::after, &:focus-visible::after': { bgcolor: colorBlue },
        // A short grip in the middle shows the gap can be dragged.
        '&::before': {
          content: '""',
          position: 'absolute',
          borderRadius: 1,
          bgcolor: colorInk20,
          ...(horizontal ? { width: 2, height: 28 } : { height: 2, width: 28 }),
        },
        position: 'relative',
      }}
    />
  );
}
