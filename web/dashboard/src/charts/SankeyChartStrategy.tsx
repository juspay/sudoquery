import React from 'react';
import { ResponsiveSankey } from '@nivo/sankey';
import { Alert, Box, Typography } from '@mui/material';
import { Info } from 'lucide-react';
import type { IChartStrategy, DataRow, CompatibilityResult, SingleChartConfig, ValidationResult } from '../types/chart';
import { getTextColor, getTooltipBackground, getTooltipTextColor } from '../utils/chartTheme';

/**
 * Represents a detected cycle in the graph
 */
interface Cycle {
  path: string[];
}

/**
 * Detects circular dependencies in a directed graph using DFS
 * @param edges Array of edges as [source, target] pairs
 * @returns Array of cycles found (each cycle is an array of node names)
 */
function detectCircularDependencies(
  edges: [string, string][],
): Cycle[] {
  // Build adjacency list
  const graph = new Map<string, string[]>();
  const nodes = new Set<string>();

  edges.forEach(([source, target]) => {
    nodes.add(source);
    nodes.add(target);

    if (!graph.has(source)) {
      graph.set(source, []);
    }
    graph.get(source)!.push(target);
  });

  const cycles: Cycle[] = [];
  const visited = new Set<string>();
  const recursionStack = new Set<string>();
  const path: string[] = [];

  /**
   * Depth-first search to detect cycles
   */
  const dfs = (node: string): void => {
    visited.add(node);
    recursionStack.add(node);
    path.push(node);

    const neighbors = graph.get(node) || [];
    for (const neighbor of neighbors) {
      if (!visited.has(neighbor)) {
        dfs(neighbor);
      } else if (recursionStack.has(neighbor)) {
        // Found a cycle - extract it from the path
        const cycleStartIndex = path.indexOf(neighbor);
        const cyclePath = [...path.slice(cycleStartIndex), neighbor];
        cycles.push({ path: cyclePath });
      }
    }

    path.pop();
    recursionStack.delete(node);
  };

  // Run DFS from each unvisited node
  nodes.forEach((node) => {
    if (!visited.has(node)) {
      dfs(node);
    }
  });

  return cycles;
}

/**
 * Finds minimal cycles by removing redundant cycles
 * e.g., if we have A->B->C->A and A->B->A, only return the shorter one
 */
function getMinimalCycles(cycles: Cycle[]): Cycle[] {
  if (cycles.length === 0) return [];

  // Deduplicate cycles by their normalized string representation
  const seen = new Set<string>();
  const unique: Cycle[] = [];

  for (const cycle of cycles) {
    // Normalize: rotate to start with lexicographically smallest node
    const normalized = normalizeCycle(cycle.path);
    const key = normalized.join('|');

    if (!seen.has(key)) {
      seen.add(key);
      unique.push(cycle);
    }
  }

  // Sort by length (shortest first)
  return unique.sort((a, b) => a.path.length - b.path.length);
}

/**
 * Normalizes a cycle by rotating it to start with the lexicographically smallest node
 * This helps identify equivalent cycles (e.g., A->B->C->A is same as B->C->A->B)
 */
function normalizeCycle(path: string[]): string[] {
  if (path.length === 0) return path;

  // Find the index of the lexicographically smallest node
  let minIndex = 0;
  for (let i = 1; i < path.length; i++) {
    if (path[i] < path[minIndex]) {
      minIndex = i;
    }
  }

  // Rotate to start from the smallest node
  return [...path.slice(minIndex), ...path.slice(0, minIndex)];
}

/**
 * Breaks cycles in the graph by removing edges with smallest values
 * @param links Array of links with source, target, and value
 * @returns Object with cleaned links and count of removed edges
 */
function breakCycles(
  links: { source: string; target: string; value: number }[]
): { cleanedLinks: typeof links; removedCount: number } {
  // Create a mutable copy
  let workingLinks = [...links];
  let removedCount = 0;
  const maxIterations = links.length; // Safety limit

  for (let i = 0; i < maxIterations; i++) {
    // Build edges from current working links
    const edges: [string, string][] = workingLinks.map((l) => [l.source, l.target]);
    const cycles = detectCircularDependencies(edges);
    const minimalCycles = getMinimalCycles(cycles);

    if (minimalCycles.length === 0) {
      break; // No more cycles
    }

    // Find all edges that are part of cycles and remove the one with smallest value
    const cycleEdges = new Set<string>();
    for (const cycle of minimalCycles) {
      for (let j = 0; j < cycle.path.length - 1; j++) {
        const edgeKey = `${cycle.path[j]}|${cycle.path[j + 1]}`;
        cycleEdges.add(edgeKey);
      }
    }

    // Find the cycle edge with smallest value
    let smallestValue = Infinity;
    let edgeToRemove: string | null = null;

    for (const edgeKey of cycleEdges) {
      const [src, tgt] = edgeKey.split('|');
      const link = workingLinks.find((l) => l.source === src && l.target === tgt);
      if (link && link.value < smallestValue) {
        smallestValue = link.value;
        edgeToRemove = edgeKey;
      }
    }

    if (edgeToRemove) {
      const [src, tgt] = edgeToRemove.split('|');
      workingLinks = workingLinks.filter((l) => !(l.source === src && l.target === tgt));
      removedCount++;
    }
  }

  return { cleanedLinks: workingLinks, removedCount };
}

/**
 * Functional component for rendering the Sankey chart with cycle breaking
 */
/* eslint-disable react-refresh/only-export-components */
function SankeyChartWithCycleDetection({
  sankeyData,
  formatNumber,
}: {
  sankeyData: { nodes: { id: string }[]; links: { source: string; target: string; value: number }[] };
  formatNumber: (value: number) => string;
}) {
  // Break cycles if present
  const { cleanedLinks, removedCount } = breakCycles(sankeyData.links);
  const hadCycles = removedCount > 0;

  // Rebuild nodes list from cleaned links (some nodes may no longer be connected)
  const connectedNodes = new Set<string>();
  cleanedLinks.forEach((link) => {
    connectedNodes.add(link.source);
    connectedNodes.add(link.target);
  });
  const cleanedNodes = sankeyData.nodes.filter((n) => connectedNodes.has(n.id));

  const cleanedData = { nodes: cleanedNodes, links: cleanedLinks };

  return (
    <Box>
      {hadCycles && (
        <Alert severity="info" icon={<Info size={20} />} sx={{ mb: 2 }}>
          <Typography variant="body2">
            {removedCount} cyclic flow{removedCount > 1 ? 's' : ''} removed to render the diagram.
          </Typography>
        </Alert>
      )}

      <div style={{ height: '100%', minHeight: 800 }}>
        <ResponsiveSankey
          data={cleanedData}
          margin={{ top: 20, right: 100, bottom: 20, left: 100 }}
          align="justify"
          sort="input"
          colors={['#264653', '#2a9d8f', '#e9c46a', '#f4a261', '#e76f51', '#a8dadc', '#457b9d', '#1d3557', '#e63946', '#f1faee']}
          nodeOpacity={0.8}
          nodeHoverOthersOpacity={0.35}
          nodeThickness={18}
          nodeSpacing={24}
          nodeBorderWidth={0}
          nodeBorderColor={{ from: 'color', modifiers: [['darker', 0.8]] }}
          linkOpacity={0.5}
          linkHoverOthersOpacity={0.1}
          linkContract={3}
          enableLinkGradient={true}
          labelPosition="outside"
          labelOrientation="horizontal"
          labelPadding={16}
          labelTextColor={getTextColor()}
          labelFormat={(value) => formatNumber(value)}
          tooltip={({ link }) => (
            <div
              style={{
                padding: '12px',
                background: getTooltipBackground(),
                color: getTooltipTextColor(),
                borderRadius: '4px',
                fontSize: '12px',
                boxShadow: '0 2px 8px rgba(0,0,0,0.2)',
              }}
            >
              <div style={{ fontWeight: 'bold', marginBottom: '4px' }}>
                {link.source.id} → {link.target.id}
              </div>
              <div>Value: {formatNumber(link.value)}</div>
            </div>
          )}
          theme={{
            tooltip: {
              container: {
                background: getTooltipBackground(),
                color: getTooltipTextColor(),
                fontFamily: "'JetBrains Mono', monospace",
                fontSize: 12,
              },
            },
          }}
        />
      </div>
    </Box>
  );
}

export class SankeyChartStrategy implements IChartStrategy {
  id = 'sankey-chart';
  name = 'Sankey Diagram';
  description = 'Best for flows, funnels, and user journeys.';

  // Format number with thousands separator
  private formatNumber(value: number): string {
    return value.toLocaleString();
  }

  validateConfig(data: DataRow[], explicitConfig?: SingleChartConfig): ValidationResult {
    const columns = data.length > 0 ? Object.keys(data[0]) : [];
    const missingFields: string[] = [];
    const invalidColumns: string[] = [];

    const valueAxis = explicitConfig?.valueAxis;

    if (!valueAxis) missingFields.push('valueAxis');
    else if (!columns.includes(valueAxis)) invalidColumns.push('valueAxis');

    return { valid: missingFields.length === 0 && invalidColumns.length === 0, missingFields, invalidColumns };
  }

  checkCompatibility(data: DataRow[]): CompatibilityResult {
    if (!data || data.length === 0) {
      return { isCompatible: false, confidence: 0 };
    }

    const sample = data[0];
    const keys = Object.keys(sample);

    const stringKeys = keys.filter((k) => typeof sample[k] === 'string');
    const numberKey = keys.find((k) => typeof sample[k] === 'number');

    // Needs 2 strings (Source/Target) and 1 number (Weight)
    if (stringKeys.length >= 2 && numberKey) {
      // Intelligent guessing based on column names
      const source =
        stringKeys.find((k) => /source|from|start|step_?1/i.test(k)) ||
        stringKeys[0];
      const target =
        stringKeys.find((k) => /target|to|end|step_?2/i.test(k)) ||
        stringKeys[1];

      // Ensure we don't pick the same column twice
      if (source !== target) {
        return {
          isCompatible: true,
          confidence: 0.95,
          matchedKeys: { source, target, value: numberKey },
        };
      }
    }

    return { isCompatible: false, confidence: 0 };
  }

  render(data: DataRow[], config: CompatibilityResult, explicitConfig?: import('../types/chart').SingleChartConfig): React.ReactNode {
    // Use explicit config if provided, otherwise fall back to auto-detected matchedKeys
    // For sankey: labelAxis typically maps to source, xAxis to target
    const source = explicitConfig?.labelAxis || explicitConfig?.xAxis || config.matchedKeys?.source;
    const target = explicitConfig?.yAxis || (explicitConfig?.labelAxis && explicitConfig?.xAxis ? explicitConfig?.xAxis : config.matchedKeys?.target);
    const value = explicitConfig?.valueAxis || config.matchedKeys?.value;

    if (!source || !target || !value) {
      return (
        <div style={{ color: '#666', textAlign: 'center', padding: '20px' }}>
          Cannot render sankey chart: missing axis configuration.
        </div>
      );
    }

    // Get unique nodes
    const rawNodes = new Set<string>();
    data.forEach((row) => {
      rawNodes.add(String(row[source]));
      rawNodes.add(String(row[target]));
    });
    const nodesList = Array.from(rawNodes);

    // Map to Nivo Sankey format (uses string IDs, not indices)
    const sankeyData = {
      nodes: nodesList.map((name) => ({ id: name })),
      links: data.map((row) => ({
        source: String(row[source]),
        target: String(row[target]),
        value: Number(row[value]),
      })),
    };

    return (
      <SankeyChartWithCycleDetection
        sankeyData={sankeyData}
        formatNumber={this.formatNumber.bind(this)}
      />
    );
  }
}
