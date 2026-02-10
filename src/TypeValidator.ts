import { JSONSerializable } from './types';

function containsNonPrimitives(obj: JSONSerializable): boolean {
  if (obj === null || typeof obj !== 'object') {
    return false;
  }

  // Arrays at the top level are allowed (return false)
  if (Array.isArray(obj)) {
    return false;
  }

  // Check object properties for nested objects or arrays
  for (const key in obj) {
    if (Object.prototype.hasOwnProperty.call(obj, key)) {
      const value = obj[key];
      if (typeof value === 'object' && value !== null) {
        return true;
      }
    }
  }

  return false;
}

export { containsNonPrimitives };
export type { JSONSerializable } from './types';
