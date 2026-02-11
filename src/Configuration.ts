class Configuration {
  private static _batchSize: number = 10;
  private static _flushInterval: number | null = null;
  private static _endpoint: string = "http://localhost:3000/push_batch";

  static get batchSize(): number {
    return Configuration._batchSize;
  }

  static get flushInterval(): number | null {
    return Configuration._flushInterval;
  }

  static get endpoint(): string {
    return Configuration._endpoint;
  }

  static setBatchSize(value: number): void {
    Configuration._batchSize = value;
  }

  static setFlushInterval(value: number | null): void {
    Configuration._flushInterval = value;
  }

  static setEndpoint(value: string): void {
    Configuration._endpoint = value;
  }
}

export { Configuration };
