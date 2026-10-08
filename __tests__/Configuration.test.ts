import { Configuration } from '../src/Configuration';

describe('Configuration', () => {
  beforeEach(() => {
    // Reset to default before each test
    Configuration.reset();
  });

  describe('batchSize getter', () => {
    it('should return default value of 10', () => {
      expect(Configuration.batchSize).toBe(10);
    });

    it('should return the value that was set', () => {
      Configuration.setBatchSize(20);
      expect(Configuration.batchSize).toBe(20);
    });
  });

  describe('batchSize setter', () => {
    it('should allow setting custom values', () => {
      Configuration.setBatchSize(5);
      expect(Configuration.batchSize).toBe(5);

      Configuration.setBatchSize(100);
      expect(Configuration.batchSize).toBe(100);

      Configuration.setBatchSize(1);
      expect(Configuration.batchSize).toBe(1);
    });

    it('should allow setting to zero', () => {
      Configuration.setBatchSize(0);
      expect(Configuration.batchSize).toBe(0);
    });

    it('should allow setting to negative values', () => {
      Configuration.setBatchSize(-10);
      expect(Configuration.batchSize).toBe(-10);
    });

    it('should allow setting to very large numbers', () => {
      Configuration.setBatchSize(999999);
      expect(Configuration.batchSize).toBe(999999);
    });
  });

  describe('static behavior', () => {
    it('should maintain state across different test contexts', () => {
      Configuration.setBatchSize(42);
      expect(Configuration.batchSize).toBe(42);
    });

    it('should not require instantiation', () => {
      // Should work without new Configuration()
      expect(Configuration.batchSize).toBeDefined();
    });
  });

  describe('headers getter', () => {
    it('should return default value of empty object', () => {
      expect(Configuration.headers).toEqual({});
    });

    it('should return the value that was set', () => {
      Configuration.setHeaders({ 'X-Custom-Header': 'test-value' });
      expect(Configuration.headers).toEqual({ 'X-Custom-Header': 'test-value' });
    });
  });

  describe('headers setter', () => {
    it('should allow setting custom headers', () => {
      Configuration.setHeaders({ 'X-Api-Key': 'my-key' });
      expect(Configuration.headers).toEqual({ 'X-Api-Key': 'my-key' });
    });

    it('should allow setting multiple headers', () => {
      Configuration.setHeaders({
        'X-Api-Key': 'my-key',
        'X-Request-Id': '12345',
      });
      expect(Configuration.headers).toEqual({
        'X-Api-Key': 'my-key',
        'X-Request-Id': '12345',
      });
    });

    it('should allow resetting headers to empty object', () => {
      Configuration.setHeaders({ 'X-Api-Key': 'my-key' });
      Configuration.setHeaders({});
      expect(Configuration.headers).toEqual({});
    });
  });

  describe('endpoint getter', () => {
    it('should return the collector batch endpoint by default', () => {
      expect(Configuration.endpoint).toBe('http://localhost:3000/batch');
    });

    it('should return the value that was set', () => {
      Configuration.setEndpoint('https://analytics.example.com/batch');
      expect(Configuration.endpoint).toBe('https://analytics.example.com/batch');
    });
  });

  describe('collector identity config', () => {
    it('should default tenant and workspace to null', () => {
      expect(Configuration.tenantId).toBeNull();
      expect(Configuration.workspaceId).toBeNull();
    });

    it('should allow setting tenant and workspace IDs', () => {
      Configuration.setTenantId('tenant-1');
      Configuration.setWorkspaceId('workspace-1');

      expect(Configuration.tenantId).toBe('tenant-1');
      expect(Configuration.workspaceId).toBe('workspace-1');
    });

    it('should default source to typescript', () => {
      expect(Configuration.source).toBe('typescript');
    });

    it('should allow overriding source and sessionId', () => {
      Configuration.setSource('checkout');
      Configuration.setSessionId('session-1');

      expect(Configuration.source).toBe('checkout');
      expect(Configuration.sessionId).toBe('session-1');
    });
  });

  describe('retry delays', () => {
    it('should default to 1s base and 60s max', () => {
      expect(Configuration.retryBaseDelay).toBe(1000);
      expect(Configuration.retryMaxDelay).toBe(60000);
    });

    it('should allow overriding and reset to defaults', () => {
      Configuration.setRetryBaseDelay(250);
      Configuration.setRetryMaxDelay(5000);

      expect(Configuration.retryBaseDelay).toBe(250);
      expect(Configuration.retryMaxDelay).toBe(5000);

      Configuration.reset();
      expect(Configuration.retryBaseDelay).toBe(1000);
      expect(Configuration.retryMaxDelay).toBe(60000);
    });
  });
});
