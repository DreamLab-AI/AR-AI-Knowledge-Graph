// Quick script to analyze edge endpoint issues
console.log("Analyzing edge endpoints for potential white block causes:");

// Simulate edge point generation
const edges = [
  {source: "A", target: "B"},
  {source: "B", target: "C"},
  {source: "A", target: "C"},
  {source: "B", target: "D"}
];

const nodes = {
  "A": {x: 0, y: 0, z: 0},
  "B": {x: 1, y: 0, z: 0},
  "C": {x: 0.5, y: 1, z: 0},
  "D": {x: 1, y: 0, z: 0} // Same as B - potential issue
};

// Count endpoint occurrences
const endpointCount = {};
edges.forEach(edge => {
  const sourceKey = `${nodes[edge.source].x},${nodes[edge.source].y},${nodes[edge.source].z}`;
  const targetKey = `${nodes[edge.target].x},${nodes[edge.target].y},${nodes[edge.target].z}`;
  
  endpointCount[sourceKey] = (endpointCount[sourceKey] || 0) + 1;
  endpointCount[targetKey] = (endpointCount[targetKey] || 0) + 1;
});

console.log("\nEndpoint occurrence count:");
Object.entries(endpointCount).forEach(([coord, count]) => {
  if (count > 1) {
    console.log(`  ${coord}: ${count} occurrences - POTENTIAL Z-FIGHTING\!`);
  }
});

console.log("\nPotential issues with THREE.LineSegments:");
console.log("1. Each segment is rendered independently");
console.log("2. Overlapping endpoints can cause z-fighting");
console.log("3. LineBasicMaterial doesn't support proper caps");
console.log("4. Depth buffer precision issues at segment boundaries");
