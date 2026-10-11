import { Configuration } from "./Configuration";
import { flush } from "./Flush";
import type { Event } from "./types";

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
   * Reset all internal state. Useful for testing.
   */
  static reset() {
    this.batches = [[]];
    this._currentBatchToUpload = 0;
    this.currentAccumilatingBatch = 0;
  }
}
