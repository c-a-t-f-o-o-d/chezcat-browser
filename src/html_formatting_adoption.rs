//! Physical parent links for adoption-agency moves. Most documents never
//! take this path. Build the index on the first block move and discard it
//! with the parser, before publishing the native document.
use super::*;

const UNKNOWN: usize = usize::MAX;
const DETACHED: usize = usize::MAX - 1;

#[derive(Default)]
pub(in crate::html) struct Topology {
    parents: Vec<usize>,
    work: usize,
}

impl Topology {
    pub(in crate::html) fn linked(&mut self, node: usize, parent: usize) -> Result<(), String> {
        if self.parents.is_empty() {
            return Ok(());
        }
        if node >= self.parents.len() {
            self.parents
                .try_reserve(node + 1 - self.parents.len())
                .map_err(|_| "Cannot reserve parser parent links.")?;
            self.parents.resize(node + 1, UNKNOWN);
        }
        self.parents[node] = parent;
        Ok(())
    }
    fn charge(&mut self, units: usize) -> Result<(), String> {
        self.work = self
            .work
            .checked_add(units)
            .filter(|n| *n <= crate::javascript::limits::BROWSER.host_work)
            .ok_or("HTML adoption tree work limit reached.")?;
        Ok(())
    }
    fn initialize(&mut self, doc: &Document, limits: LoadedTree<'_>) -> Result<(), String> {
        if !self.parents.is_empty() {
            return Ok(());
        }
        self.charge(doc.nodes.len())?;
        self.parents
            .try_reserve_exact(doc.nodes.len())
            .map_err(|_| "Cannot reserve parser parent index.")?;
        self.parents.resize(doc.nodes.len(), UNKNOWN);
        self.parents[0] = DETACHED;
        for (parent, node) in doc.nodes.iter().enumerate() {
            if parent % 512 == 0 {
                (limits.check_cancel)()?;
            }
            for &child in &node.children {
                if self.parents[child] != UNKNOWN {
                    return Err("Duplicate native child before HTML adoption.".into());
                }
                self.parents[child] = parent;
            }
        }
        Ok(())
    }
    fn parent(
        &mut self,
        doc: &Document,
        node: usize,
        limits: LoadedTree<'_>,
    ) -> Result<usize, String> {
        match self.parents.get(node).copied() {
            Some(p) if p != UNKNOWN => return Ok(p),
            _ => {}
        }
        // Structural nodes created after the first repair can be encountered
        // as ancestors. Cache their actual edge on first use.
        for (parent, n) in doc.nodes.iter().enumerate() {
            if parent % 512 == 0 {
                (limits.check_cancel)()?;
            }
            self.charge(1 + n.children.len())?;
            if n.children.contains(&node) {
                self.linked(node, parent)?;
                return Ok(parent);
            }
        }
        Err("Missing native parent during HTML adoption.".into())
    }
    fn detach(
        &mut self,
        doc: &mut Document,
        node: usize,
        limits: LoadedTree<'_>,
    ) -> Result<(), String> {
        let parent = self.parent(doc, node, limits)?;
        if parent == DETACHED {
            return Ok(());
        }
        let children = &mut doc.nodes[parent].children;
        if children.last() == Some(&node) {
            children.pop();
        } else {
            self.charge(children.len())?;
            let at = children
                .iter()
                .position(|&n| n == node)
                .ok_or("Missing native adoption edge.")?;
            children.remove(at);
        }
        self.linked(node, DETACHED)
    }
    fn attach(
        &mut self,
        doc: &mut Document,
        node: usize,
        parent: usize,
        mut foster: Option<&mut table_construction::Tables>,
        limits: LoadedTree<'_>,
    ) -> Result<(), String> {
        let parent = if let Some(t) = foster.as_deref_mut() {
            t.foster_location(doc)?.0
        } else {
            parent
        };
        let mut ancestor = parent;
        while ancestor != DETACHED {
            self.charge(1)?;
            if ancestor == node {
                return Err("HTML adoption would create a cyclic native tree.".into());
            }
            ancestor = self.parent(doc, ancestor, limits)?;
        }
        self.detach(doc, node, limits)?;
        if let Some(t) = foster {
            t.link_before_table(doc, node)?;
        } else {
            let children = &mut doc.nodes[parent].children;
            children
                .try_reserve(1)
                .map_err(|_| "Cannot reserve adopted child.")?;
            children.push(node);
        }
        self.linked(node, parent)
    }
    pub(in crate::html) fn finish(
        &mut self,
        doc: &mut Document,
        fragment: bool,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<(), String> {
        if self.parents.is_empty() {
            return Ok(());
        }
        // A block move can change physical depth independently of the open
        // stack. Walk actual edges before exposing the result to consumers.
        let mut path = Vec::new();
        path.try_reserve_exact(MAX_DEPTH + 1)
            .map_err(|_| "Cannot verify adopted tree depth.")?;
        path.push((0usize, 0usize));
        // Fragment publication consumes parent-before-child records for
        // depth, resources, native forms and owner-preserving insertion.
        // Reuse the parser-only index for the old-to-preorder permutation.
        if fragment {
            self.linked(doc.nodes.len() - 1, UNKNOWN)?;
            self.parents.fill(UNKNOWN);
            self.parents[0] = 0;
        }
        let mut visited = 1;
        while let Some((parent, next)) = path.last_mut() {
            let Some(&child) = doc.nodes[*parent].children.get(*next) else {
                path.pop();
                continue;
            };
            *next += 1;
            if path.len() > MAX_DEPTH {
                return Err(format!(
                    "HTML document nesting limit reached ({MAX_DEPTH} open nodes)."
                ));
            }
            if fragment {
                if self.parents[child] != UNKNOWN {
                    return Err("Duplicate native fragment edge.".into());
                }
                self.parents[child] = visited;
            }
            visited += 1;
            if visited % 512 == 0 {
                check()?;
            }
            if visited > doc.nodes.len() {
                return Err("Invalid HTML adoption topology.".into());
            }
            path.push((child, 0));
        }
        if visited != doc.nodes.len() {
            return Err("Unreachable native node after HTML adoption.".into());
        }
        if fragment {
            let order = &mut self.parents;
            for node in &mut doc.nodes {
                for child in &mut node.children {
                    *child = order[*child];
                }
            }
            for flags in [&mut doc.nonce_unsafe_nodes, &mut doc.withheld_inline_styles] {
                if flags.is_empty() {
                    continue;
                }
                let mut moved = HashSet::new();
                moved
                    .try_reserve(flags.len())
                    .map_err(|_| "Cannot remap fragment resource flags.")?;
                moved.extend(flags.iter().map(|&node| order[node]));
                *flags = moved;
            }
            // All IDs are private staging indices. No DOM identity has been
            // allocated yet. Swap node records in place without cloning them.
            for i in 0..order.len() {
                while order[i] != i {
                    let j = order[i];
                    doc.nodes.swap(i, j);
                    order.swap(i, j);
                }
            }
        }
        Ok(())
    }
}

impl Formatting {
    pub(super) fn adopt_block(
        &mut self,
        doc: &mut Document,
        stack: &mut Vec<usize>,
        index: usize,
        block_depth: usize,
        tables: &mut table_construction::Tables,
        limits: LoadedTree<'_>,
    ) -> Result<(), String> {
        (limits.check_cancel)()?;
        self.topology.initialize(doc, limits)?;
        let original = self.entries[index].node;
        let depth = self.entries[index].depth;
        let block = stack[block_depth];
        let common = stack[depth - 1];
        let mut last = block;
        let mut bookmark = index;
        let mut moved_bookmark = false;
        let mut inner = 0;
        for pos in ((depth + 1)..block_depth).rev() {
            inner += 1;
            let old = stack[pos];
            let active = self.entries[self.start()..]
                .iter()
                .position(|e| e.node == old)
                .map(|i| self.start() + i);
            let Some(i) = active else {
                stack.remove(pos);
                continue;
            };
            if inner > 3 {
                self.entries.remove(i);
                if i < bookmark {
                    bookmark -= 1;
                }
                stack.remove(pos);
                continue;
            }
            let clone = clone_formatting(&mut self.copied, doc, old, limits.nodes)?;
            self.topology.linked(clone, DETACHED)?;
            stack[pos] = clone;
            self.entries[i] = Entry {
                node: clone,
                depth: pos,
            };
            if !moved_bookmark {
                bookmark = i + 1;
                moved_bookmark = true;
            }
            self.topology.attach(doc, last, clone, None, limits)?;
            last = clone;
        }
        let foster = table_construction::foster_target(doc, common) && tables.has_active(stack);
        self.topology
            .attach(doc, last, common, foster.then_some(tables), limits)?;
        let replacement = clone_formatting(&mut self.copied, doc, original, limits.nodes)?;
        let children = std::mem::take(&mut doc.nodes[block].children);
        self.topology.charge(children.len())?;
        for (i, &child) in children.iter().enumerate() {
            if i % 512 == 0 {
                (limits.check_cancel)()?;
            }
            self.topology.linked(child, replacement)?;
        }
        doc.nodes[replacement].children = children;
        doc.nodes[block]
            .children
            .try_reserve(1)
            .map_err(|_| "Cannot reserve adopted formatting edge.")?;
        doc.nodes[block].children.push(replacement);
        self.topology.linked(replacement, block)?;
        let at = self
            .entries
            .iter()
            .rposition(|e| e.node == original)
            .ok_or("Missing original formatting entry.")?;
        self.entries.remove(at);
        if at < bookmark {
            bookmark -= 1;
        }
        self.entries.insert(
            bookmark,
            Entry {
                node: replacement,
                depth: usize::MAX,
            },
        );
        stack.remove(depth);
        let at = stack
            .iter()
            .position(|&n| n == block)
            .ok_or("Missing adopted block on parser stack.")?;
        stack.insert(at + 1, replacement);
        let start = self.start();
        for e in &mut self.entries[start..] {
            e.depth = stack
                .iter()
                .position(|&n| n == e.node)
                .unwrap_or(usize::MAX);
        }
        Ok(())
    }
}
