export interface AnimatedIconHandle {
  startAnimation: () => void | Promise<void>;
  stopAnimation: () => void | Promise<void>;
}
