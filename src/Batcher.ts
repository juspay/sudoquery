import { Configuration } from "./Configuration";
import { flush } from "./Flush";
import { Event } from "./types";
import { getStorage, getPlatform, isPlatformInitialized } from "./platform";

const PENDING_EVENTS_KEY = 'hyper_analytics_pending_events';

export class Batcher {
  private static batches: Array<Array<Event>> = [[]];
  private static _currentBatchToUpload = 0;
  private static currentAccumilatingBatch = 0;

  static addToBatch(event : Event){
    if(this.batches[this.currentAccumilatingBatch].length === Configuration.batchSize){
      this.addNewBatch();
    }
    const lastBatch = this.batches[this.batches.length - 1];
    lastBatch.push(event);
    // Auto-flush when batch is full, but don't await it to avoid blocking
    // Don't use keepalive for regular flushes (only use during page unload)
    if(lastBatch.length === Configuration.batchSize) {
      flush(false).catch(err => console.error('Auto-flush error:', err));
    }
  }

  static fetchBatchToUpload(): (Event[] | null) {
    const batchToUpload = this.batches[this._currentBatchToUpload];
    if(batchToUpload.length == 0) return null;
    if(this._currentBatchToUpload === this.currentAccumilatingBatch){
      this.addNewBatch();
    }
    return batchToUpload;
  }

  static addNewBatch(){
    this.batches.push([]);
    this.currentAccumilatingBatch = this.batches.length - 1;
  }

  static setMarkLastBatchUploaded() {
    return this._currentBatchToUpload++;
  }

  /**
   * Get all pending events (for persistence).
   */
  static getAllPendingEvents(): Event[] {
    const allEvents: Event[] = [];
    for (const batch of this.batches) {
      allEvents.push(...batch);
    }
    return allEvents;
  }

  /**
   * Clear all pending events (after successful persistence).
   */
  static clearAllEvents(): void {
    this.batches = [[]];
    this._currentBatchToUpload = 0;
    this.currentAccumilatingBatch = 0;
  }

  /**
   * Persist pending events to storage (for React Native termination handling).
   */
  static async persistBatch(): Promise<void> {
    if (!isPlatformInitialized()) return;

    const platform = getPlatform();
    if (platform !== 'react-native') return;

    const storage = getStorage();
    const pendingEvents = this.getAllPendingEvents();

    if (pendingEvents.length > 0) {
      await storage.setItem(PENDING_EVENTS_KEY, JSON.stringify(pendingEvents));
    }
  }

  /**
   * Restore pending events from storage (on app launch).
   */
  static async restoreBatch(): Promise<void> {
    if (!isPlatformInitialized()) return;

    const platform = getPlatform();
    if (platform !== 'react-native') return;

    const storage = getStorage();
    const pending = await storage.getItem(PENDING_EVENTS_KEY);

    if (pending) {
      try {
        const events: Event[] = JSON.parse(pending);
        for (const event of events) {
          this.addToBatch(event);
        }
        // Clear persisted events after restoration
        await storage.removeItem(PENDING_EVENTS_KEY);
      } catch (error) {
        console.error('Failed to restore pending events:', error);
      }
    }
  }

  /**
   * Reset all internal state. Useful for testing.
   */
  static reset() {
    this.batches = [[]];
    this._currentBatchToUpload = 0;
    this.currentAccumilatingBatch = 0;
  }
}