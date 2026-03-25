// Mock for @react-native-async-storage/async-storage
const storage = new Map<string, string>();

export default {
  getItem: jest.fn((key: string) => Promise.resolve(storage.get(key) ?? null)),
  setItem: jest.fn((key: string, value: string) => {
    storage.set(key, value);
    return Promise.resolve();
  }),
  removeItem: jest.fn((key: string) => {
    storage.delete(key);
    return Promise.resolve();
  }),
};
