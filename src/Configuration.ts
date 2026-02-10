class Configuration {
  private static _batchSize: number = 10;

  static get batchSize(): number {
    return Configuration._batchSize;
  }

  static set batchSize(value: number) {
    Configuration._batchSize = value;
  }
}

export { Configuration };
