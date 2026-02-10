/**
 * AnonymousId manages the generation and persistence of anonymous user IDs.
 *
 * - Browser: IDs persist across browser sessions using localStorage
 * - Node.js: IDs persist only for the current application session (in-memory)
 */

const STORAGE_KEY = 'hyper_analytics_anon_id';

export class AnonymousId {
  // In-memory storage for Node.js environment
  private static inMemoryAnonId: string | null = null;

  /**
   * Generates a new UUID v4 using crypto.randomUUID()
   */
  private static generateId(): string {
    return crypto.randomUUID();
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
      // Browser: use localStorage
      let anonId = localStorage.getItem(STORAGE_KEY);

      if (!anonId) {
        anonId = this.generateId();
        localStorage.setItem(STORAGE_KEY, anonId);
      }

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
      localStorage.removeItem(STORAGE_KEY);
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
      return localStorage.getItem(STORAGE_KEY);
    } else {
      return this.inMemoryAnonId;
    }
  }
}