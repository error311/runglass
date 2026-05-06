function buildTree(processes) {
  const nodes = new Map(processes.map((process) => [process.pid, { ...process, children: [] }]));
  const roots = [];
  for (const node of nodes.values()) {
    if (node.ppid && nodes.has(node.ppid)) {
      nodes.get(node.ppid).children.push(node);
    } else {
      roots.push(node);
    }
  }
  return roots;
}
