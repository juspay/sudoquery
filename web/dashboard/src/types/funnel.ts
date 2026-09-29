interface FunnelStep {
  id: string;
  eventName: string;
}

interface FunnelConfig {
  startDate: string;
  endDate: string;
  steps: FunnelStep[];
}

interface FunnelResult {
  level: number;
  count: number;
}