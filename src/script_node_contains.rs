//! Node.contains over validated identities and their real physical trees.
//! https://dom.spec.whatwg.org/#dom-node-contains
use super::*;

pub(super) const CONTAINS_METHOD: &str = "\0Node.contains";

/// The binding layer validates current owner/origin and genuine Node brands.
/// An Attr's ownerElement is deliberately absent from this tree identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ContainsNode {
    Main(usize),
    Attr,
    Child { owner: u32, node: usize },
}
type ContainsArguments = (ContainsNode, Option<(u32, ContainsNode)>);

impl Bridge<'_> {
    /// Nullable Node conversion never reads author properties or coerces values.
    /// Receiver validation precedes even null and self shortcuts, including for
    /// retained functions whose child Window has since become inaccessible.
    pub(super) fn validate_node_contains(
        &self,
        object: u32,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        self.contains_arguments(object, args).map(|_| ())
    }

    fn contains_arguments(
        &self,
        object: u32,
        args: &[Value],
    ) -> Result<ContainsArguments, (&'static str, String)> {
        let this = self.contains_node_identity(object)?;
        match args.first() {
            None => Err(("TypeError", "Node.contains requires a nullable Node argument.".into())),
            Some(Value::Null | Value::Undefined) => Ok((this, None)),
            Some(Value::Host(other)) => self.contains_node_identity(*other)
                .map(|that| (this, Some((*other, that)))),
            _ => Err(("TypeError", "Node.contains requires a genuine Node or null.".into())),
        }
    }

    pub(super) fn node_contains(&mut self, object: u32, args: &[Value]) -> Result<Value, String> {
        let (this, other) = self.contains_arguments(object, args).map_err(|(_, message)| message)?;
        self.state.work(1)?;
        let Some((other, that)) = other else {
            // Validation has already distinguished explicit nullish values
            // from missing/invalid arguments and checked receiver authority.
            return Ok(Value::Bool(false));
        };
        let result = match (this, that) {
            (ContainsNode::Attr, ContainsNode::Attr) => object == other,
            (ContainsNode::Attr, _) | (_, ContainsNode::Attr) => false,
            (ContainsNode::Main(this), ContainsNode::Main(other)) => {
                self.state.rebuild(self.doc)?;
                let (this_root, _) = self.contains_main_ancestry(this, None)?;
                let (other_root, found) = self.contains_main_ancestry(other, Some(this))?;
                this_root == other_root && found
            }
            (
                ContainsNode::Child { owner, node: this },
                ContainsNode::Child { owner: other_owner, node: other },
            ) if owner == other_owner => {
                // Blank child documents have no shared main-tree parent map.
                // Charge the complete bounded physical scans before walking;
                // no temporary path, parent map or author callback is needed.
                let count = self.contains_child_document(owner)?.nodes.len();
                self.state.work(count)?;
                let edges = self.contains_child_document(owner)?.nodes.iter()
                    .fold(0usize, |total, node| total.saturating_add(node.children.len()));
                let work = count.saturating_mul(count.saturating_add(edges).saturating_add(1))
                    .saturating_mul(2);
                self.state.work(work)?;
                let doc = self.contains_child_document(owner)?;
                child_ancestry(doc, this, None)?;
                child_ancestry(doc, other, Some(this))?
            }
            // A browsing-context embedding is not a physical DOM parent edge.
            // Both identities were validated before this disconnected result.
            _ => false,
        };
        Ok(Value::Bool(result))
    }

    /// Continue to the real root even after finding the receiver, so a later
    /// broken edge or cycle cannot turn malformed topology into success.
    fn contains_main_ancestry(
        &mut self,
        mut node: usize,
        sought: Option<usize>,
    ) -> Result<(usize, bool), String> {
        let mut found = false;
        for _ in 0..self.doc.nodes.len() {
            self.state.work(1)?;
            if node >= self.doc.nodes.len() {
                return Err("Invalid Node.contains tree index.".into());
            }
            found |= sought == Some(node);
            let parent = *self.state.tree.parents.get(node)
                .ok_or("Invalid Node.contains parent index.")?;
            if node == 0 {
                if parent.is_some() {
                    return Err("Main Document cannot have a Node.contains parent.".into());
                }
                // rebuild_dom_tree starts at (0, None) and rejects every
                // repeated connected visit. Guarded tree transactions retain
                // that root invariant and publish parent/child changes before
                // marking the cache dirty. Reaching it needs no global scan;
                // every traversed edge and the full path were checked above.
                return Ok((0, found));
            }
            let Some(parent) = parent else {
                // A detached root must really have no physical parent. This
                // also rejects a missing cached edge rather than certifying a
                // nominal self/ancestor match from inconsistent metadata.
                self.state.work(self.doc.nodes.len())?;
                for entry in &self.doc.nodes {
                    self.state.work(entry.children.len())?;
                    if entry.children.iter().any(|&child| child == node || child >= self.doc.nodes.len()) {
                        return Err("Node.contains root/physical parent mismatch.".into());
                    }
                }
                return Ok((node, found));
            };
            let children = &self.doc.nodes.get(parent)
                .ok_or("Invalid Node.contains physical parent.")?.children;
            self.state.work(children.len())?;
            if children.iter().filter(|&&child| child == node).count() != 1 {
                return Err("Node.contains parent/child identity mismatch.".into());
            }
            node = parent;
        }
        Err("Cyclic or unavailable Node.contains ancestry.".into())
    }
}

/// Allocation-free child traversal after the caller prepays both full walks.
/// Search actual child lists, require exactly one parent, and refuse a retained
/// disconnected child context rather than substituting the embedding iframe.
fn child_ancestry(doc: &Document, mut node: usize, sought: Option<usize>) -> Result<bool, String> {
    let mut found = false;
    for _ in 0..doc.nodes.len() {
        if node >= doc.nodes.len() {
            return Err("Invalid child Node.contains tree index.".into());
        }
        found |= sought == Some(node);
        let mut parent = None;
        for (index, entry) in doc.nodes.iter().enumerate() {
            for &child in &entry.children {
                if child >= doc.nodes.len() {
                    return Err("Invalid child Node.contains physical edge.".into());
                }
                if child == node {
                    if parent.replace(index).is_some() {
                        return Err("Child Node.contains requires one physical parent edge.".into());
                    }
                }
            }
        }
        if node == 0 {
            if parent.is_some() {
                return Err("Child Document cannot have a Node.contains parent.".into());
            }
            return Ok(found);
        }
        node = parent.ok_or("Disconnected child Node.contains metadata is unavailable.")?;
    }
    Err("Cyclic child Node.contains ancestry.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_contains_warmed_connected_query_cost_does_not_scan_unrelated_descendants() {
        let mut costs = Vec::new();
        for unrelated in [0, 512] {
            let mut doc = crate::html::parse(&format!(
                "<html><head></head><body><div id=query><span id=leaf></span></div><div>{}</div></body></html>",
                "<span>unrelated</span>".repeat(unrelated)
            ));
            doc.scripting_enabled = true;
            let mut forms = Forms::new(&doc);
            let mut page = Page::for_test();
            assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0, "{}", page.report());
            page.begin_job();
            page.state.rebuild(&doc).unwrap();
            let find = |name: &str| (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap();
            let (query, leaf) = (find("query"), find("leaf"));
            let before = page.state.host_work;
            let commits = page.state.dom_commits;
            let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
            assert!(matches!(bridge.node_contains(node_handle(query), &[Value::Host(node_handle(leaf))]).unwrap(), Value::Bool(true)));
            costs.push(before - bridge.state.host_work);
            assert_eq!(bridge.state.dom_commits, commits);
        }
        assert!(costs[0] > 0);
        assert_eq!(costs[0], costs[1], "Unrelated connected subtrees do not belong to an ancestor walk");
    }

    #[test]
    fn node_contains_child_physical_walk_is_inclusive_and_rejects_duplicate_broken_and_cyclic_edges() {
        let doc = crate::html::parse("<html><head></head><body><div id=parent><span id=child></span></div></body></html>");
        let id = |name: &str| (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap();
        let (parent, child) = (id("parent"), id("child"));
        assert!(child_ancestry(&doc, child, Some(child)).unwrap());
        assert!(child_ancestry(&doc, child, Some(parent)).unwrap());
        assert!(child_ancestry(&doc, child, Some(0)).unwrap());
        assert!(!child_ancestry(&doc, parent, Some(child)).unwrap());
        let mut duplicate = crate::html::parse("<html><body></body></html>");
        let child = duplicate.nodes[0].children[0];
        duplicate.nodes[0].children.push(child);
        assert!(child_ancestry(&duplicate, child, None).is_err());
        assert!(child_ancestry(&duplicate, child, Some(child)).is_err());
        let mut broken = crate::html::parse("<html><body></body></html>");
        let invalid = broken.nodes.len();
        broken.nodes[0].children.push(invalid);
        assert!(child_ancestry(&broken, 0, None).is_err());
        let mut cycle = crate::html::parse("<html><body></body></html>");
        let child = cycle.nodes[0].children[0];
        cycle.nodes[child].children.push(0);
        assert!(child_ancestry(&cycle, child, None).is_err());
        assert!(child_ancestry(&cycle, child, Some(child)).is_err());
    }
}
