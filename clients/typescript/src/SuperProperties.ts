import type { JSONSerializable } from "./types";

export class SuperProperties {
  private static properties: Record<string, JSONSerializable> = {};

  static addToSuperProperties(key: string, value: JSONSerializable): void {
    this.properties[key] = value;
  }

  static getSuperProperties(): Record<string, JSONSerializable> {
    return { ...this.properties };
  }

  static clearSuperProperties(): void {
    this.properties = {};
  }
}
