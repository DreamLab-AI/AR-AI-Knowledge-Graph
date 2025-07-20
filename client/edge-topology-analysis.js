// Edge Topology Analysis for White Block Issue
// This script analyzes how many edges share endpoints in a typical graph

// Sample graph structure similar to what's used in the app
const sampleGraph = {
  nodes: [
    { id: 'A', label: 'Node A' },
    { id: 'B', label: 'Node B' },
    { id: 'C', label: 'Node C' },
    { id: 'D', label: 'Node D' },
    { id: 'E', label: 'Node E' },
    { id: 'F', label: 'Node F' }
  ],
  edges: [
    // Node A is a hub - connected to many nodes
    { source: 'A', target: 'B' },
    { source: 'A', target: 'C' },
    { source: 'A', target: 'D' },
    { source: 'A', target: 'E' },
    // Node B has multiple connections
    { source: 'B', target: 'C' },
    { source: 'B', target: 'D' },
    // Node C connections
    { source: 'C', target: 'F' },
    // Node D connections
    { source: 'D', target: 'E' },
    { source: 'D', target: 'F' }
  ]
};

// Analyze endpoint duplications
function analyzeEdgeTopology(graph) {
  const endpointCounts = {};
  const nodeConnections = {};
  
  // Count how many times each node appears as an endpoint
  graph.edges.forEach(edge => {
    // Count source endpoints
    endpointCounts[edge.source] = (endpointCounts[edge.source] || 0) + 1;
    // Count target endpoints
    endpointCounts[edge.target] = (endpointCounts[edge.target] || 0) + 1;
    
    // Track connections per node
    if (!nodeConnections[edge.source]) nodeConnections[edge.source] = [];
    if (!nodeConnections[edge.target]) nodeConnections[edge.target] = [];
    nodeConnections[edge.source].push(edge.target);
    nodeConnections[edge.target].push(edge.source);
  });
  
  // Calculate edge points array size
  const totalEdgePoints = graph.edges.length * 2; // Each edge creates 2 points
  const edgePointsArraySize = totalEdgePoints * 3; // Each point has x,y,z
  
  // Find nodes with multiple connections (potential white block locations)
  const highDegreeNodes = Object.entries(endpointCounts)
    .filter(([node, count]) => count > 2)
    .sort((a, b) => b[1] - a[1]);
  
  console.log('Edge Topology Analysis:');
  console.log('======================');
  console.log(`Total nodes: ${graph.nodes.length}`);
  console.log(`Total edges: ${graph.edges.length}`);
  console.log(`Edge points array size: ${edgePointsArraySize} values (${totalEdgePoints} 3D points)`);
  console.log('\nEndpoint duplication count per node:');
  
  Object.entries(endpointCounts)
    .sort((a, b) => b[1] - a[1])
    .forEach(([node, count]) => {
      console.log(`  Node ${node}: appears ${count} times as endpoint (${count} overlapping edge endpoints)`);
    });
  
  console.log('\nHigh-degree nodes (likely white block locations):');
  highDegreeNodes.forEach(([node, count]) => {
    console.log(`  Node ${node}: ${count} connections`);
  });
  
  // Calculate overlap statistics
  const totalDuplicates = Object.values(endpointCounts).reduce((sum, count) => sum + count, 0);
  const uniqueEndpoints = Object.keys(endpointCounts).length;
  const duplicateRatio = totalDuplicates / totalEdgePoints;
  
  console.log('\nOverlap Statistics:');
  console.log(`  Total endpoint instances: ${totalDuplicates}`);
  console.log(`  Unique endpoint positions: ${uniqueEndpoints}`);
  console.log(`  Average duplicates per position: ${(totalDuplicates / uniqueEndpoints).toFixed(2)}`);
  console.log(`  Duplication ratio: ${(duplicateRatio * 100).toFixed(1)}%`);
  
  return {
    endpointCounts,
    nodeConnections,
    highDegreeNodes,
    statistics: {
      totalNodes: graph.nodes.length,
      totalEdges: graph.edges.length,
      totalEndpoints: totalDuplicates,
      uniqueEndpoints,
      averageDuplicates: totalDuplicates / uniqueEndpoints
    }
  };
}

// Run analysis
const analysis = analyzeEdgeTopology(sampleGraph);

console.log('\nConclusion:');
console.log('The white blocks appear at nodes with multiple edge connections because:');
console.log('1. Each edge creates 2 endpoints in the edgePoints array');
console.log('2. When multiple edges connect to the same node, that node\'s position is duplicated');
console.log('3. These duplicate positions create overlapping geometry at render time');
console.log('4. The overlapping transparent geometry creates bright white artifacts');