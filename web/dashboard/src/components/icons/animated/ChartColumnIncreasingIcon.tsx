"use client";

import type { Variants } from 'motion/react';
import { motion, useAnimation } from 'motion/react';
import type { HTMLAttributes } from 'react';
import { forwardRef, useCallback, useImperativeHandle, useRef } from 'react';

import type { AnimatedIconHandle } from './types';

interface ChartColumnIncreasingIconProps extends HTMLAttributes<HTMLDivElement> {
  size?: number;
}

const lineVariants: Variants = {
  visible: { pathLength: 1, opacity: 1 },
  hidden: { pathLength: 0, opacity: 0 },
};

export const ChartColumnIncreasingIcon = forwardRef<AnimatedIconHandle, ChartColumnIncreasingIconProps>(
  ({ onMouseEnter, onMouseLeave, className, size = 20, style, ...props }, ref) => {
    const controls = useAnimation();
    const isControlledRef = useRef(false);

    useImperativeHandle(ref, () => {
      isControlledRef.current = true;

      return {
        startAnimation: async () => {
          await controls.start((index) => ({
            pathLength: 0,
            opacity: 0,
            transition: { delay: index * 0.1, duration: 0.3 },
          }));
          await controls.start((index) => ({
            pathLength: 1,
            opacity: 1,
            transition: { delay: index * 0.1, duration: 0.3 },
          }));
        },
        stopAnimation: () => controls.start('visible'),
      };
    });

    const handleMouseEnter = useCallback(
      async (event: React.MouseEvent<HTMLDivElement>) => {
        if (isControlledRef.current) {
          onMouseEnter?.(event);
          return;
        }

        await controls.start((index) => ({
          pathLength: 0,
          opacity: 0,
          transition: { delay: index * 0.1, duration: 0.3 },
        }));
        await controls.start((index) => ({
          pathLength: 1,
          opacity: 1,
          transition: { delay: index * 0.1, duration: 0.3 },
        }));
      },
      [controls, onMouseEnter],
    );

    const handleMouseLeave = useCallback(
      (event: React.MouseEvent<HTMLDivElement>) => {
        if (isControlledRef.current) {
          onMouseLeave?.(event);
          return;
        }

        controls.start('visible');
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
        <svg
          fill="none"
          height={size}
          stroke="currentColor"
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth="2"
          viewBox="0 0 24 24"
          width={size}
          xmlns="http://www.w3.org/2000/svg"
        >
          <motion.path animate={controls} custom={1} d="M13 17V9" initial="visible" variants={lineVariants} />
          <motion.path animate={controls} custom={2} d="M18 17V5" initial="visible" variants={lineVariants} />
          <path d="M3 3v16a2 2 0 0 0 2 2h16" />
          <motion.path animate={controls} custom={0} d="M8 17v-3" initial="visible" variants={lineVariants} />
        </svg>
      </div>
    );
  },
);

ChartColumnIncreasingIcon.displayName = 'ChartColumnIncreasingIcon';
