"use client";

import { motion, useAnimation } from 'motion/react';
import type { HTMLAttributes } from 'react';
import { forwardRef, useCallback, useImperativeHandle, useRef } from 'react';

import type { AnimatedIconHandle } from './types';

interface TerminalIconProps extends HTMLAttributes<HTMLDivElement> {
  size?: number;
}

export const TerminalIcon = forwardRef<AnimatedIconHandle, TerminalIconProps>(
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
            normal: { scale: 1 },
            animate: {
              scale: 1.05,
              transition: {
                duration: 0.3,
                ease: 'easeOut',
              },
            },
          }}
          viewBox="0 0 24 24"
          width={size}
          xmlns="http://www.w3.org/2000/svg"
        >
          <rect x="4" y="4" width="16" height="16" rx="2" ry="2" />
          <motion.path
            d="M7 8l4 4-4 4"
            stroke="currentColor"
            strokeWidth="2"
            variants={{
              normal: {
                pathLength: 1,
                opacity: 1,
              },
              animate: {
                pathLength: [1, 0, 1],
                opacity: [1, 0.5, 1],
                transition: {
                  duration: 0.7,
                  delay: 0.3,
                },
              },
            }}
          />
          <motion.path
            d="M13 16h4"
            stroke="currentColor"
            strokeWidth="2"
            variants={{
              normal: {
                pathLength: 1,
                opacity: 1,
              },
              animate: {
                pathLength: [1, 0, 1],
                opacity: [1, 0.5, 1],
                transition: {
                  duration: 0.7,
                  delay: 0.5,
                },
              },
            }}
          />
        </motion.svg>
      </div>
    );
  },
);

TerminalIcon.displayName = 'TerminalIcon';
