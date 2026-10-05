"use client";

import { motion, useAnimation } from 'motion/react';
import type { HTMLAttributes } from 'react';
import { forwardRef, useCallback, useImperativeHandle, useRef } from 'react';

import type { AnimatedIconHandle } from './types';

interface SearchIconProps extends HTMLAttributes<HTMLDivElement> {
  size?: number;
}

export const SearchIcon = forwardRef<AnimatedIconHandle, SearchIconProps>(
  ({ onMouseEnter, onMouseLeave, className, size = 18, style, ...props }, ref) => {
    const controls = useAnimation();
    const isControlledRef = useRef(false);

    useImperativeHandle(ref, () => {
      isControlledRef.current = true;

      return {
        startAnimation: () => controls.start('animate'),
        stopAnimation: () => controls.start('normal'),
      };
    });

    const handleMouseEnter = useCallback(
      (event: React.MouseEvent<HTMLDivElement>) => {
        if (isControlledRef.current) {
          onMouseEnter?.(event);
          return;
        }

        controls.start('animate');
      },
      [controls, onMouseEnter],
    );

    const handleMouseLeave = useCallback(
      (event: React.MouseEvent<HTMLDivElement>) => {
        if (isControlledRef.current) {
          onMouseLeave?.(event);
          return;
        }

        controls.start('normal');
      },
      [controls, onMouseLeave],
    );

    return (
      <div
        className={className}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
        style={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          width: size,
          height: size,
          lineHeight: 0,
          flexShrink: 0,
          ...style,
        }}
        {...props}
      >
        <motion.svg
          animate={controls}
          fill="none"
          height={size}
          initial="normal"
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="2"
          variants={{
            normal: { x: 0, y: 0 },
            animate: {
              x: [0, 0, -2, 0],
              y: [0, -2, 0, 0],
              transition: {
                duration: 0.8,
                ease: 'easeInOut',
              },
            },
          }}
          viewBox="0 0 24 24"
          width={size}
          xmlns="http://www.w3.org/2000/svg"
        >
          <circle cx="11" cy="11" r="7" />
          <path d="m20 20-4-4" />
        </motion.svg>
      </div>
    );
  },
);

SearchIcon.displayName = 'SearchIcon';
