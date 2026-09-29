import { BarChartStrategy } from './BarChartStrategy';
import { LineChartStrategy } from './LineChartStrategy';
import { SankeyChartStrategy } from './SankeyChartStrategy';
import { PieChartStrategy } from './PieChartStrategy';
import { FunnelChartStrategy } from './FunnelChartStrategy';
import { SegmentedMatrixStrategy } from './SegmentedMatrix';
import type { IChartStrategy, DataRow, CompatibilityResult } from './IChartStrategy';

const chartStrategies: IChartStrategy[] = [
  new BarChartStrategy(),
  new LineChartStrategy(),
  new SankeyChartStrategy(),
  new PieChartStrategy(),
  new FunnelChartStrategy(),
  new SegmentedMatrixStrategy(),
];

export function getCompatibleCharts(data: DataRow[]): Array<{
  strategy: IChartStrategy;
  config: CompatibilityResult;
}> {
  return chartStrategies
    .map((strategy) => ({
      strategy,
      config: strategy.checkCompatibility(data),
    }))
    .filter(({ config }) => config.isCompatible)
    .sort((a, b) => b.config.confidence - a.config.confidence);
}

export function getStrategyById(id: string): IChartStrategy | undefined {
  return chartStrategies.find((s) => s.id === id);
}