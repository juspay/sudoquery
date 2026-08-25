/**
 * AnonymousId manages the generation and persistence of anonymous user IDs.
 *
 * - Browser: IDs persist across browser sessions using localStorage
 * - Node.js: IDs persist only for the current application session (in-memory)
 */

import { generateUuid } from "./Uuid";

const STORAGE_KEY = 'hyper_analytics_anon_id';

export class AnonymousId {
  // In-memory storage for Node.js environment
  private static inMemoryAnonId: string | null = null;

  private static generateId(): string {
    return generateUuid();
  }

  /**
   * Checks if running in browser environment
   */
  private static isBrowser(): boolean {
    return typeof window !== 'undefined';
  }

  /**
   * Gets the current anonymous ID.
   * - Browser: retrieves from localStorage, creates if not exists
   * - Node.js: retrieves from in-memory storage, creates if not exists
   *
   * @returns The anonymous ID string
   */
  public static getOrCreate(): string {
    if (this.isBrowser()) {
      const storedAnonId = this.getFromStorage();
      if (storedAnonId) return storedAnonId;

      const anonId = this.generateId();
      this.setInStorage(anonId);
      return anonId;
    } else {
      // Node.js: use in-memory storage
      if (!this.inMemoryAnonId) {
        this.inMemoryAnonId = this.generateId();
      }

      return this.inMemoryAnonId;
    }
  }

  /**
   * Resets the anonymous ID.
   * - Browser: removes from localStorage
   * - Node.js: clears in-memory storage
   * The next call to getOrCreate() will generate a new ID.
   */
  public static reset(): void {
    if (this.isBrowser()) {
      this.removeFromStorage();
    } else {
      this.inMemoryAnonId = null;
    }
  }

  /**
   * Gets the current anonymous ID without creating a new one if it doesn't exist.
   * - Browser: reads from localStorage
   * - Node.js: reads from in-memory storage
   *
   * @returns The anonymous ID string, or null if not set
   */
  public static get(): string | null {
    if (this.isBrowser()) {
      return this.getFromStorage();
    } else {
      return this.inMemoryAnonId;
    }
  }

  private static getFromStorage(): string | null {
    try {
      return localStorage.getItem(STORAGE_KEY);
    } catch (_) {
      return this.inMemoryAnonId;
    }
  }

  private static setInStorage(anonId: string): void {
    try {
      localStorage.setItem(STORAGE_KEY, anonId);
    } catch (_) {
      this.inMemoryAnonId = anonId;
    }
  }

  private static removeFromStorage(): void {
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch (_) {
      // Storage can be unavailable in sandboxed iframes.
    }
    this.inMemoryAnonId = null;
  }
}
