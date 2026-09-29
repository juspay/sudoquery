export function generateUuid(): string {
  const runtimeCrypto = getRuntimeCrypto();

  if (runtimeCrypto && typeof runtimeCrypto.randomUUID === "function") {
    return runtimeCrypto.randomUUID();
  }

  const bytes = new Uint8Array(16);
  if (runtimeCrypto && typeof runtimeCrypto.getRandomValues === "function") {
    runtimeCrypto.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) {
      bytes[i] = Math.floor(Math.random() * 256);
    }
  }

  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0"));
  return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex.slice(6, 8).join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10, 16).join("")}`;
}

function getRuntimeCrypto(): Crypto | null {
  return typeof globalThis !== "undefined" && "crypto" in globalThis ? globalThis.crypto : null;
}
