class Configuration {
  private static readonly DEFAULT_ENDPOINT = "http://localhost:3000/batch";
  private static readonly DEFAULT_SOURCE = "typescript";

  private static _batchSize: number = 10;
  private static _flushInterval: number | null = null;
  private static _endpoint: string = Configuration.DEFAULT_ENDPOINT;
  private static _token: string | null = null;
  private static _headers: Record<string, string> = {};
  private static _tenantId: string | null = null;
  private static _workspaceId: string | null = null;
  private static _source: string | null = Configuration.DEFAULT_SOURCE;
  private static _sessionId: string | null = null;

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

  static get tenantId(): string | null {
    return Configuration._tenantId;
  }

  static get workspaceId(): string | null {
    return Configuration._workspaceId;
  }

  static get source(): string | null {
    return Configuration._source;
  }

  static get sessionId(): string | null {
    return Configuration._sessionId;
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

  static setTenantId(value: string | null): void {
    Configuration._tenantId = value;
  }

  static setWorkspaceId(value: string | null): void {
    Configuration._workspaceId = value;
  }

  static setSource(value: string | null): void {
    Configuration._source = value;
  }

  static setSessionId(value: string | null): void {
    Configuration._sessionId = value;
  }

  static reset(): void {
    Configuration._batchSize = 10;
    Configuration._flushInterval = null;
    Configuration._endpoint = Configuration.DEFAULT_ENDPOINT;
    Configuration._token = null;
    Configuration._headers = {};
    Configuration._tenantId = null;
    Configuration._workspaceId = null;
    Configuration._source = Configuration.DEFAULT_SOURCE;
    Configuration._sessionId = null;
  }
}

export { Configuration };
