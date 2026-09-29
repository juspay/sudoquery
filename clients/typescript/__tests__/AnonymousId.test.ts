import { AnonymousId } from "../src/AnonymousId";

describe("AnonymousId", () => {
  const globalsToRestore = ["window", "localStorage", "crypto"] as const;
  let originalDescriptors: Partial<Record<(typeof globalsToRestore)[number], PropertyDescriptor>>;

  beforeEach(() => {
    originalDescriptors = {};
    for (const key of globalsToRestore) {
      originalDescriptors[key] = Object.getOwnPropertyDescriptor(globalThis, key);
    }
    AnonymousId.reset();
  });

  afterEach(() => {
    for (const key of globalsToRestore) {
      const descriptor = originalDescriptors[key];
      if (descriptor) {
        Object.defineProperty(globalThis, key, descriptor);
      } else {
        delete (globalThis as Record<string, unknown>)[key];
      }
    }
    AnonymousId.reset();
  });

  it("uses in-memory IDs when browser storage and crypto are unavailable", () => {
    Object.defineProperty(globalThis, "window", {
      value: {},
      configurable: true,
    });
    Object.defineProperty(globalThis, "crypto", {
      value: undefined,
      configurable: true,
    });
    Object.defineProperty(globalThis, "localStorage", {
      value: {
        getItem() {
          throw new Error("blocked storage");
        },
        setItem() {
          throw new Error("blocked storage");
        },
        removeItem() {
          throw new Error("blocked storage");
        },
      },
      configurable: true,
    });

    const firstAnonId = AnonymousId.getOrCreate();
    const secondAnonId = AnonymousId.getOrCreate();

    expect(firstAnonId).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
    );
    expect(secondAnonId).toBe(firstAnonId);
    expect(() => AnonymousId.reset()).not.toThrow();
  });
});
