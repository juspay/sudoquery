import { Batcher } from "./Batcher";
import { Pusher } from "./Pusher";

export async function flush(useBeacon: boolean = false) {
  while(true){
    const res = await Pusher.pushLogs(useBeacon);
    if(res === null){
      return;
    } else {
      Batcher.setMarkLastBatchUploaded();
    }
  }
}
