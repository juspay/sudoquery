// MetricsPage — wraps the existing SavedMetrics component.
import { useState } from 'react';
import { SavedMetrics } from '../components/SavedMetrics';
import { RightPanel } from '../components/Chat';
import { executeQuery } from '../services/clickhouseService';
import type { SavedMetric } from '../types/metric';
import { trackMetricDeleted } from '../utils/analytics';

export default function MetricsPage() {
  const [savedMetrics, setSavedMetrics] = useState<SavedMetric[]>(() => {
    try {
      const data = localStorage.getItem('hyper_analytics_saved_metrics');
      return data ? JSON.parse(data) as SavedMetric[] : [];
    } catch { return []; }
  });

  const [metricResult, setMetricResult] = useState<{
    label: string; description?: string; query: string; response: unknown;
    savedMetricId?: string; dynamicQuery?: any; chartConfig?: any;
  } | null>(null);
  const [rightPanelOpen, setRightPanelOpen] = useState(false);

  const handleViewMetric = async (metric: SavedMetric) => {
    setMetricResult({ label: metric.label, description: metric.description, query: metric.query, response: null, savedMetricId: metric.id, dynamicQuery: metric.dynamicQuery, chartConfig: metric.chartConfig });
    setRightPanelOpen(true);
    try {
      const result = await executeQuery(metric.query);
      setMetricResult((prev) => prev ? { ...prev, response: result } : null);
    } catch { /* silent */ }
  };

  const handleDeleteMetric = (id: string) => {
    try {
      const data = localStorage.getItem('hyper_analytics_saved_metrics');
      const metrics: SavedMetric[] = data ? JSON.parse(data) : [];
      const toDelete = metrics.find((m) => m.id === id);
      const filtered = metrics.filter((m) => m.id !== id);
      localStorage.setItem('hyper_analytics_saved_metrics', JSON.stringify(filtered));
      setSavedMetrics(filtered);
      if (toDelete) trackMetricDeleted(id, toDelete.label);
    } catch { /* silent */ }
  };

  const handleRefreshMetrics = () => {
    try {
      const data = localStorage.getItem('hyper_analytics_saved_metrics');
      setSavedMetrics(data ? JSON.parse(data) : []);
    } catch { /* silent */ }
  };

  return (
    <>
      <SavedMetrics
        metrics={savedMetrics}
        onViewMetric={handleViewMetric}
        onDeleteMetric={handleDeleteMetric}
        onMetricCreated={handleRefreshMetrics}
      />
      {metricResult && (
        <RightPanel
          isOpen={rightPanelOpen}
          onClose={() => { setRightPanelOpen(false); setMetricResult(null); }}
          query={metricResult.query}
          response={metricResult.response}
          label={metricResult.label}
          description={metricResult.description}
          onRefresh={async () => {
            if (!metricResult) return;
            setMetricResult((prev) => prev ? { ...prev, response: null } : null);
            try {
              const result = await executeQuery(metricResult.query);
              setMetricResult((prev) => prev ? { ...prev, response: result } : null);
            } catch { /* silent */ }
          }}
          savedMetricId={metricResult.savedMetricId}
          dynamicQuery={metricResult.dynamicQuery}
          chartConfig={metricResult.chartConfig}
        />
      )}
    </>
  );
}
