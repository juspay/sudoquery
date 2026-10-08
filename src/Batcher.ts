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

  /**
   * Remove every pending batch for a page-unload send. When keepHead is true the
   * head batch is already being uploaded, so it stays queued for that upload to mark.
   */
  static takeAllPending(keepHead: boolean): Event[][] {
    const pending = this.batches.splice(keepHead ? 1 : 0).filter(batch => batch.length > 0);
    this.batches.push([]);
    return pending;
  }

  /**
   * Put back a batch whose unload send failed so the next flush retries it.
   * When afterHead is true it goes behind the batch currently being uploaded.
   */
  static requeue(batch: Event[], afterHead: boolean) {
    this.batches.splice(afterHead ? 1 : 0, 0, batch);
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
