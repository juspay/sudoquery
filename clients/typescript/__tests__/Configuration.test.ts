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
    it('should default org and project to null', () => {
      expect(Configuration.orgId).toBeNull();
      expect(Configuration.projectId).toBeNull();
    });

    it('should allow setting org and project IDs', () => {
      Configuration.setOrgId('acme-org-1');
      Configuration.setProjectId('acme-project-1');

      expect(Configuration.orgId).toBe('acme-org-1');
      expect(Configuration.projectId).toBe('acme-project-1');
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
});
