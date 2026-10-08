import { Configuration } from "./Configuration";
import { flush } from "./Flush";
import { Event } from "./types";

export class Batcher {
  // Queue of pending batches: the head is the next to upload, the tail is accumulating.
  // Uploaded batches are removed so memory stays bounded by what is still unsent.
  private static batches: Array<Array<Event>> = [[]];

  static addToBatch(event : Event){
    if(this.accumulatingBatch().length >= Configuration.batchSize){
      this.addNewBatch();
    }
    const lastBatch = this.accumulatingBatch();
    lastBatch.push(event);
    // Auto-flush when batch is full, but don't await it to avoid blocking
    // Don't use keepalive for regular flushes (only use during page unload)
    if(lastBatch.length === Configuration.batchSize) {
      flush(false).catch(err => console.error('Auto-flush error:', err));
    }
  }

  static fetchBatchToUpload(): (Event[] | null) {
    const batchToUpload = this.batches[0];
    if(batchToUpload.length == 0) return null;
    // Stop accumulating into the batch being uploaded
    if(this.batches.length === 1){
      this.addNewBatch();
    }
    return batchToUpload;
  }

  static addNewBatch(){
    this.batches.push([]);
  }

  static setMarkLastBatchUploaded() {
    this.batches.shift();
    if(this.batches.length === 0){
      this.batches.push([]);
    }
  }

  private static accumulatingBatch(): Event[] {
    return this.batches[this.batches.length - 1];
  }

  /**
   * Reset all internal state. Useful for testing.
   */
  static reset() {
    this.batches = [[]];
  }
}
