// Mock for react-native-device-info
export default {
  getDeviceType: jest.fn(() => Promise.resolve('Handset')),
  getSystemName: jest.fn(() => 'iOS'),
  getSystemVersion: jest.fn(() => '17.0'),
  getVersion: jest.fn(() => '1.0.0'),
  getBuildNumber: jest.fn(() => '1'),
};
