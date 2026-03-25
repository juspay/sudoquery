/**
 * AnonymousId manages the generation and persistence of anonymous user IDs.
 *
 * - Browser: IDs persist across browser sessions using localStorage
 * - React Native: IDs persist using AsyncStorage
 * - Node.js: IDs persist only for the current application session (in-memory)
 */

import { getStorage, isPlatformInitialized } from './platform';

const STORAGE_KEY = 'hyper_analytics_anon_id';

export class AnonymousId {
  // In-memory cache for all environments
  private static cachedAnonId: string | null = null;
  private static initPromise: Promise<void> | null = null;

  /**
   * Generates a new UUID v4.
   * Works in browser, React Native, and Node.js.
   */
  private static generateId(): string {
    // crypto.randomUUID is available in modern browsers, Node.js 16.7+, and React Native with polyfill
    if (typeof crypto !== 'undefined' && crypto.randomUUID) {
      return crypto.randomUUID();
    }
    // Fallback UUID generation
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
      const r = (Math.random() * 16) | 0;
      const v = c === 'x' ? r : (r & 0x3) | 0x8;
      return v.toString(16);
    });
  }

  /**
   * Initialize the anonymous ID from storage.
   * Must be called before getOrCreate() in React Native.
   */
  static async initialize(): Promise<void> {
    if (this.initPromise) return this.initPromise;

    this.initPromise = (async () => {
      // Check if we already have a cached ID
      if (this.cachedAnonId) return;

      // If platform is initialized, use storage adapter
      if (isPlatformInitialized()) {
        try {
          const storage = getStorage();
          let anonId = await storage.getItem(STORAGE_KEY);

          if (!anonId) {
            anonId = this.generateId();
            await storage.setItem(STORAGE_KEY, anonId);
          }

          this.cachedAnonId = anonId;
        } catch {
          // Fallback to memory-only if storage fails
          this.cachedAnonId = this.generateId();
        }
      } else {
        // Fallback for cases where platform isn't initialized (shouldn't happen normally)
        this.cachedAnonId = this.generateId();
      }
    })();

    return this.initPromise;
  }

  /**
   * Gets the current anonymous ID.
   * Synchronous version - returns cached ID.
   *
   * @returns The anonymous ID string
   * @throws Error if not initialized (call init() first)
   */
  public static getOrCreate(): string {
    if (this.cachedAnonId) return this.cachedAnonId;
    throw new Error(
      'AnonymousId not initialized. Call HyperAnalytics.init() first.'
    );
  }

  /**
   * Gets the current anonymous ID (async version).
   * Initializes if not already done.
   *
   * @returns The anonymous ID string
   */
  public static async getOrCreateAsync(): Promise<string> {
    await this.initialize();
    return this.cachedAnonId!;
  }

  /**
   * Resets the anonymous ID.
   * Removes from storage and clears memory.
   */
  public static async reset(): Promise<void> {
    this.cachedAnonId = null;
    this.initPromise = null;

    if (isPlatformInitialized()) {
      try {
        const storage = getStorage();
        await storage.removeItem(STORAGE_KEY);
      } catch {
        // Ignore errors if storage is not available
      }
    }
  }

  /**
   * Sync reset - clears memory only.
   * Used when resetting without async context.
   */
  public static resetSync(): void {
    this.cachedAnonId = null;
    this.initPromise = null;
  }

  /**
   * Gets the current anonymous ID without creating a new one if it doesn't exist.
   *
   * @returns The anonymous ID string, or null if not set
   */
  public static get(): string | null {
    return this.cachedAnonId;
  }
}