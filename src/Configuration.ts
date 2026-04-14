class Configuration {
  private static _batchSize: number = 10;
  private static _flushInterval: number | null = null;
  private static _endpoint: string = "https://sudoquery.juspay.io/api/push_batch";
  private static _token: string | null = null;
  private static _headers: Record<string, string> = {};

  static get batchSize(): number {
    return Configuration._batchSize;
  }

  static get flushInterval(): number | null {
    return Configuration._flushInterval;
  }

  static get endpoint(): string {
    return Configuration._endpoint;
  }

  static get token(): string | null {
    return Configuration._token;
  }

  static get headers(): Record<string, string> {
    return Configuration._headers;
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

  static setToken(value: string | null): void {
    Configuration._token = value;
  }

  static setHeaders(value: Record<string, string>): void {
    Configuration._headers = value;
  }
}

export { Configuration };
