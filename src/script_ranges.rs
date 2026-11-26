//! Native live DOM range boundaries; no interpreter values are retained here.
use super::*;
use crate::javascript::range_interface::*;
use std::cmp::Ordering;

const BASE: u32 = 0x8d00_0000;
const END: u32 = 0x8e00_0000;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Point {
    pub node: u32,
    pub offset: usize,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct Record {
    pub fixed: bool,
    pub owner: u32,
    pub start: Point,
    pub end: Point,
}

impl State {
    pub(super) fn prepare_range_changes(
        &mut self,
        operations: usize,
        nodes: usize,
    ) -> Result<(), String> {
        if self.ranges.is_empty() {
            return Ok(());
        }
        let work = self
            .ranges
            .len()
            .checked_mul(2)
            .and_then(|work| work.checked_mul(operations))
            .and_then(|work| work.checked_mul(nodes.checked_add(1)?))
            .ok_or("Live Range update work overflow.")?;
        self.work(work)
    }
    // Called only after all fallible mutation preparation, before publication.
    pub(super) fn ranges_remove(
        &mut self,
        parent: usize,
        index: usize,
        count: usize,
        removed: &[usize],
    ) {
        let parent = if parent == 0 {
            DOCUMENT
        } else {
            node_handle(parent)
        };
        for range in self.ranges.iter_mut().filter(|range|!range.fixed) {
            for point in [&mut range.start, &mut range.end] {
                if node_index(point.node).is_some_and(|node| removed.contains(&node)) {
                    *point = Point {
                        node: parent,
                        offset: index,
                    };
                } else if point.node == parent && point.offset > index {
                    point.offset -= count.min(point.offset - index);
                }
            }
        }
    }
    pub(super) fn ranges_insert(&mut self, parent: usize, index: usize, count: usize) {
        let parent = if parent == 0 {
            DOCUMENT
        } else {
            node_handle(parent)
        };
        self.ranges_insert_handle(parent,index,count);
    }
    pub(super) fn ranges_insert_handle(&mut self, parent:u32, index:usize, count:usize) {
        for range in self.ranges.iter_mut().filter(|range|!range.fixed) {
            for point in [&mut range.start, &mut range.end] {
                if point.node == parent && point.offset > index {
                    point.offset += count;
                }
            }
        }
    }
    // Offset/count/result lengths have been admitted before this infallible step.
    pub(super) fn ranges_splice_data(&mut self,node:u32,offset:usize,removed:usize,inserted:usize){
        let end=offset+removed;
        for range in self.ranges.iter_mut().filter(|range|!range.fixed) {
            for point in [&mut range.start,&mut range.end] {
                if point.node!=node {continue;}
                if point.offset>offset&&point.offset<=end {point.offset=offset;}
                else if point.offset>end {point.offset=point.offset-removed+inserted;}
            }
        }
    }
    pub(super) fn ranges_split_text(&mut self,source:u32,new_node:u32,offset:usize,parent:Option<(u32,usize)>){for range in self.ranges.iter_mut().filter(|range|!range.fixed){for point in [&mut range.start,&mut range.end]{if point.node==source&&point.offset>offset{point.node=new_node;point.offset-=offset;}else if parent.is_some_and(|(parent,gap)|point.node==parent&&point.offset==gap){point.offset+=1;}}}}
    pub(super) fn ranges_normalize_remove(&mut self,parent:u32,node:u32,index:usize){for range in self.ranges.iter_mut().filter(|range|!range.fixed){for point in [&mut range.start,&mut range.end]{if point.node==node{point.node=parent;point.offset=index;}else if point.node==parent&&point.offset>index{point.offset-=1;}}}}
    pub(super) fn ranges_normalize_merge(&mut self,parent:u32,node:u32,keeper:u32,index:usize,offset:usize){for range in self.ranges.iter_mut().filter(|range|!range.fixed){for point in [&mut range.start,&mut range.end]{if point.node==node{point.node=keeper;point.offset+=offset;}else if point.node==parent{if point.offset==index{point.node=keeper;point.offset=offset;}else if point.offset>index{point.offset-=1;}}}}}
    pub(super) fn ranges_replace_data(&mut self, node: usize) {
        for range in self.ranges.iter_mut().filter(|range|!range.fixed) {
            for point in [&mut range.start, &mut range.end] {
                if point.node == node_handle(node) {
                    point.offset = 0;
                }
            }
        }
    }

}

impl Bridge<'_> {
    pub(super) fn range_record(&self, handle: u32) -> Option<&Record> {
        let index = handle.checked_sub(BASE).filter(|_| handle < END)? as usize;
        self.state.ranges.get(index)
    }
    pub(super) fn range_owner(&self, handle: u32) -> Option<u32> {
        self.range_record(handle).map(|range| range.owner)
    }
    pub(in crate::scripting) fn range_node(&self, handle: u32) -> Result<usize, String> {
        if handle == DOCUMENT {
            return Ok(0);
        }
        node_index(handle)
            .filter(|&node| self.dom_accessible(node))
            .ok_or_else(|| {
                "Range endpoints currently require accessible displayed-document nodes.".into()
            })
    }
    pub(in crate::scripting) fn range_length(&mut self, handle: u32) -> Result<usize, String> {
        let node = self.range_node(handle)?;
        let item = &self.doc.nodes[node];
        if let Some(text) = item.kind.character_dom_string() {
            self.state.work(text.used_bytes())?;
            Ok(text.utf16_len())
        } else {
            Ok(item.children.len())
        }
    }
    pub(in crate::scripting) fn range_parent(&mut self, handle: u32) -> Result<Option<u32>, String> {
        let node = self.range_node(handle)?;
        self.state.work(1)?;
        Ok(self
            .state
            .tree
            .parents
            .get(node)
            .copied()
            .flatten()
            .map(|node| {
                if node == 0 {
                    DOCUMENT
                } else {
                    node_handle(node)
                }
            }))
    }
    pub(in crate::scripting) fn range_index(&mut self, handle: u32) -> Result<(u32, usize), String> {
        let parent = self
            .range_parent(handle)?
            .ok_or("InvalidNodeTypeError: Range relative boundary requires a parent.")?;
        let node = self.range_node(handle)?;
        let parent_index = self.range_node(parent)?;
        self.state
            .work(self.doc.nodes[parent_index].children.len())?;
        let index = self.doc.nodes[parent_index]
            .children
            .iter()
            .position(|&child| child == node)
            .ok_or("Range parent/child topology is inconsistent.")?;
        Ok((parent, index))
    }
    pub(in crate::scripting) fn range_root(&mut self, mut node: u32) -> Result<u32, String> {
        while let Some(parent) = self.range_parent(node)? {
            node = parent;
        }
        Ok(node)
    }
    pub(in crate::scripting) fn range_ancestor_child(
        &mut self,
        ancestor: u32,
        mut node: u32,
    ) -> Result<Option<u32>, String> {
        while let Some(parent) = self.range_parent(node)? {
            if parent == ancestor {
                return Ok(Some(node));
            }
            node = parent;
        }
        Ok(None)
    }
    pub(in crate::scripting) fn range_compare(&mut self, a: Point, b: Point) -> Result<Option<Ordering>, String> {
        if a.node == b.node {
            return Ok(Some(a.offset.cmp(&b.offset)));
        }
        if self.range_root(a.node)? != self.range_root(b.node)? {
            return Ok(None);
        }
        if let Some(child) = self.range_ancestor_child(a.node, b.node)? {
            return Ok(Some(if self.range_index(child)?.1 < a.offset {
                Ordering::Greater
            } else {
                Ordering::Less
            }));
        }
        if let Some(child) = self.range_ancestor_child(b.node, a.node)? {
            return Ok(Some(if self.range_index(child)?.1 < b.offset {
                Ordering::Less
            } else {
                Ordering::Greater
            }));
        }
        let mut ancestor = a.node;
        while let Some(parent) = self.range_parent(ancestor)? {
            if let Some(other) = self.range_ancestor_child(parent, b.node)? {
                return Ok(Some(
                    self.range_index(ancestor)?
                        .1
                        .cmp(&self.range_index(other)?.1),
                ));
            }
            ancestor = parent;
        }
        Err("Range boundary order is unavailable.".into())
    }
    pub(super) fn create_range(&mut self, owner: u32, document: u32) -> Result<Value, String> {
        if owner != WINDOW || document != DOCUMENT {
            return Err(
                "Range construction in child or inert documents is not implemented.".into(),
            );
        }
        let point = Point {
            node: document,
            offset: 0,
        };
        self.copy_range(Record {
            fixed: false,
            owner,
            start: point,
            end: point,
        })
    }
    fn copy_range(&mut self, record: Record) -> Result<Value, String> {
        self.state.work(1)?;
        let index = u32::try_from(self.state.ranges.len())
            .ok()
            .and_then(|index| BASE.checked_add(index))
            .filter(|&handle| handle < END)
            .ok_or("Range native identity space exhausted.")?;
        let available = self
            .state
            .retained_limit
            .saturating_sub(self.state.retained);
        let (replacement, bytes) = retained_storage::grow(&self.state.ranges, 1, available)?;
        if let Some(mut replacement) = replacement {
            self.state.copies(
                self.state
                    .ranges
                    .len()
                    .checked_mul(size_of::<Record>())
                    .ok_or("Range backing copy overflow.")?,
            )?;
            replacement.append(&mut self.state.ranges);
            self.state.ranges = replacement;
        }
        self.state.ranges.push(record);
        self.state.retained += bytes;
        Ok(Value::Host(index))
    }
    pub(super) fn range_call(
        &mut self,
        receiver: u32,
        method: &str,
        args: &[Value],
    ) -> Result<Value, String> {
        if method == RANGE {
            return self.create_range(receiver, DOCUMENT);
        }
        if method == CREATE {
            return self.create_range(WINDOW, receiver);
        }
        let mut record = *self
            .range_record(receiver)
            .ok_or("Range operation requires a genuine Range receiver.")?;
        if comparison(method) {return self.range_comparison(record,method,args);}
        if record.fixed && !snapshot_getter(method) {return Err("A live Range receiver is required.".into());}
        match method {
            START_NODE => return Ok(Value::Host(record.start.node)),
            END_NODE => return Ok(Value::Host(record.end.node)),
            START_OFFSET => return Ok(Value::Number(record.start.offset as f64)),
            END_OFFSET => return Ok(Value::Number(record.end.offset as f64)),
            COLLAPSED => return Ok(Value::Bool(record.start == record.end)),
            COMMON => {
                let mut node = record.start.node;
                while node != record.end.node
                    && self.range_ancestor_child(node, record.end.node)?.is_none()
                {
                    node = self
                        .range_parent(node)?
                        .ok_or("Range has no common ancestor.")?;
                }
                return Ok(Value::Host(node));
            }
            STRINGIFY => return self.range_string(record),
            CLONE => return self.copy_range(record),
            CLONE_CONTENTS => return self.clone_range_contents(record),
            DETACH => return Ok(Value::Undefined),
            COLLAPSE => {
                if args.first().is_some_and(Value::truthy) {
                    record.end = record.start;
                } else {
                    record.start = record.end;
                }
            }
            SET_START | SET_END | START_BEFORE | START_AFTER | END_BEFORE | END_AFTER
            | SELECT_NODE | SELECT_CONTENTS => {
                let Some(Value::Host(node)) = args.first() else {
                    return Err("A genuine Node is required for a Range boundary.".into());
                };
                self.range_node(*node)?;
                let point = if matches!(method, SET_START | SET_END) {
                    let offset = range_offset(args.get(1).unwrap_or(&Value::Undefined))?;
                    if offset > self.range_length(*node)? {
                        return Err("IndexSizeError: Range offset exceeds node length.".into());
                    }
                    Point {
                        node: *node,
                        offset,
                    }
                } else if method == SELECT_CONTENTS {
                    let length = self.range_length(*node)?;
                    record.start = Point {
                        node: *node,
                        offset: 0,
                    };
                    record.end = Point {
                        node: *node,
                        offset: length,
                    };
                    self.state.ranges[(receiver - BASE) as usize] = record;
                    return Ok(Value::Undefined);
                } else {
                    let (parent, index) = self.range_index(*node)?;
                    if method == SELECT_NODE {
                        record.start = Point {
                            node: parent,
                            offset: index,
                        };
                        record.end = Point {
                            node: parent,
                            offset: index + 1,
                        };
                        self.state.ranges[(receiver - BASE) as usize] = record;
                        return Ok(Value::Undefined);
                    }
                    Point {
                        node: parent,
                        offset: index + usize::from(matches!(method, START_AFTER | END_AFTER)),
                    }
                };
                if matches!(method, SET_START | START_BEFORE | START_AFTER) {
                    if !matches!(
                        self.range_compare(point, record.end)?,
                        Some(Ordering::Less | Ordering::Equal)
                    ) {
                        record.end = point;
                    }
                    record.start = point;
                } else {
                    if !matches!(
                        self.range_compare(point, record.start)?,
                        Some(Ordering::Greater | Ordering::Equal)
                    ) {
                        record.start = point;
                    }
                    record.end = point;
                }
            }
            _ => return Err("Range operation is outside the implemented boundary subset.".into()),
        }
        self.state.ranges[(receiver - BASE) as usize] = record;
        Ok(Value::Undefined)
    }
}

fn range_offset(value: &Value) -> Result<usize, String> {
    let number = match value {
        Value::Number(number) => *number,
        Value::Undefined => f64::NAN,
        _ => return Err("Expected a converted numeric Range offset.".into()),
    };
    Ok(if number.is_finite() {
        number.trunc().rem_euclid(4_294_967_296.0) as u32 as usize
    } else {
        0
    })
}
impl Bridge<'_> {
    pub(super) fn validate_range_call(
        &mut self,
        receiver: u32,
        method: &str,
        args: &[Value],
    ) -> Result<(), Error> {
        use Error::{Exception, Fatal};
        self.state.work(1).map_err(Fatal)?;
        self.state.rebuild(self.doc).map_err(Fatal)?;
        if matches!(method, ABSTRACT | NODE_RANGE) {
            return Err(Exception(
                "TypeError",
                "Illegal range interface constructor.".into(),
            ));
        }
        if method==STATIC {return if self.event_target_interface(receiver){Ok(())}else{Err(Exception("TypeError","StaticRange requires a Window constructor.".into()))};}
        if method == RANGE {
            return if receiver == WINDOW {
                Ok(())
            } else {
                Err(Exception(
                    "NotSupportedError",
                    "Child Range construction is not implemented.".into(),
                ))
            };
        }
        if method == CREATE {
            return if receiver == DOCUMENT {
                Ok(())
            } else {
                Err(Exception(
                    "NotSupportedError",
                    "Range creation requires the displayed document.".into(),
                ))
            };
        }
        if self.range_record(receiver).is_none() {
            return Err(Exception(
                "TypeError",
                "A genuine Range receiver is required.".into(),
            ));
        }
        if self.range_record(receiver).is_some_and(|record|record.fixed) && !snapshot_getter(method) {return Err(Exception("TypeError","A live Range receiver is required.".into()));}
        if comparison(method) {return self.validate_range_comparison(receiver,method,args);}
        if method == CLONE_CONTENTS {
            let record=*self.range_record(receiver).unwrap();
            self.state.work(self.doc.nodes.len()).map_err(Fatal)?;
            for index in 0..self.doc.nodes.len() {
                if !matches!(self.doc.nodes[index].kind,NodeKind::DocumentType(_)) {continue;}
                let handle=node_handle(index);
                if self.range_parent(handle).map_err(Fatal)?.is_some() {
                    let (parent,offset)=self.range_index(handle).map_err(Fatal)?;
                    if self.range_compare(record.start,Point{node:parent,offset}).map_err(Fatal)?==Some(Ordering::Less)
                        || record.start==(Point{node:parent,offset}) {
                        if self.range_compare(Point{node:parent,offset:offset+1},record.end).map_err(Fatal)?!=Some(Ordering::Greater) {
                            return Err(Exception("HierarchyRequestError","Range clone cannot contain a DocumentType.".into()));
                        }
                    }
                }
            }
        }
        if node_argument(method) {
            let Some(Value::Host(handle)) = args.first() else {
                return Err(Exception("TypeError", "A genuine Node is required.".into()));
            };
            if *handle != DOCUMENT
                && !node_index(*handle).is_some_and(|node| self.dom_accessible(node))
                && self.inert_node_type(*handle).is_none()
                && !self.attr_receiver(*handle)
                && self.frame_node_interface(*handle).is_none()
            {
                return Err(Exception("TypeError", "A genuine Node is required.".into()));
            }
            let node = self
                .range_node(*handle)
                .map_err(|error| Exception("NotSupportedError", error))?;
            if matches!(method,SET_START|SET_END|SELECT_CONTENTS) && matches!(self.doc.nodes[node].kind,NodeKind::DocumentType(_)) {
                return Err(Exception("InvalidNodeTypeError","A DocumentType cannot be a Range boundary.".into()));
            }
            if matches!(method, SET_START | SET_END) {
                let offset = range_offset(args.get(1).unwrap_or(&Value::Undefined))
                    .map_err(|error| Exception("TypeError", error))?;
                let length = self.range_length(*handle).map_err(Fatal)?;
                if offset > length {
                    return Err(Exception(
                        "IndexSizeError",
                        "Range offset exceeds node length.".into(),
                    ));
                }
            }
            if matches!(
                method,
                START_BEFORE | START_AFTER | END_BEFORE | END_AFTER | SELECT_NODE
            ) && self
                .state
                .tree
                .parents
                .get(node)
                .copied()
                .flatten()
                .is_none()
            {
                return Err(Exception(
                    "InvalidNodeTypeError",
                    "A relative Range boundary requires a parent.".into(),
                ));
            }
        }
        Ok(())
    }
}

impl Bridge<'_> {
    fn range_string(&mut self, range: Record) -> Result<Value, String> {
        if range.start == range.end {
            return Ok(Value::String(String::new()));
        }
        let mut common = range.start.node;
        while common != range.end.node
            && self.range_ancestor_child(common, range.end.node)?.is_none()
        {
            common = self
                .range_parent(common)?
                .ok_or("Range has no common ancestor.")?;
        }
        let root = self.range_node(common)?;
        let mut stack = Vec::new();
        stack
            .try_reserve(1)
            .map_err(|_| "Range text traversal reservation failed.")?;
        stack.push(root);
        let mut output = Vec::new();
        while let Some(node) = stack.pop() {
            self.state.work(1)?;
            if matches!(self.doc.nodes[node].kind, NodeKind::Text(_) | NodeKind::Cdata(_)) {
                let handle = node_handle(node);
                let length = self.range_length(handle)?;
                if self.range_compare(
                    Point {
                        node: handle,
                        offset: length,
                    },
                    range.start,
                )? == Some(Ordering::Less)
                    || self.range_compare(
                        Point {
                            node: handle,
                            offset: 0,
                        },
                        range.end,
                    )? == Some(Ordering::Greater)
                {
                    continue;
                }
                let start = if range.start.node == handle {
                    range.start.offset
                } else {
                    0
                };
                let end = if range.end.node == handle {
                    range.end.offset
                } else {
                    length
                };
                let count = end.saturating_sub(start);
                let bytes = count.checked_mul(2).ok_or("Range string size overflow.")?;
                let total = output
                    .len()
                    .checked_add(count)
                    .and_then(|count| count.checked_mul(2))
                    .ok_or("Range result size overflow.")?;
                if total > self.state.limits.host.string_bytes {
                    return Err("Range string exceeds the shared host string allowance.".into());
                }
                self.state.copies(bytes)?;
                output
                    .try_reserve(count)
                    .map_err(|_| "Range string reservation failed.")?;
                let (NodeKind::Text(text) | NodeKind::Cdata(text)) = &self.doc.nodes[node].kind else {
                    unreachable!()
                };
                output.extend(text.encode_utf16().skip(start).take(count));
            } else {
                let children = &self.doc.nodes[node].children;
                self.state.work(children.len())?;
                stack
                    .try_reserve(children.len())
                    .map_err(|_| "Range text traversal reservation failed.")?;
                stack.extend(children.iter().rev().copied());
            }
        }
        if std::char::decode_utf16(output.iter().copied()).any(|unit| unit.is_err()) {
            return Ok(Value::StringUnits(output));
        }
        let bytes = std::char::decode_utf16(output.iter().copied())
            .try_fold(0usize, |bytes, unit| {
                bytes.checked_add(unit.unwrap().len_utf8())
            })
            .ok_or("Range UTF8 size overflow.")?;
        if bytes > self.state.limits.host.string_bytes {
            return Err("Range string exceeds the shared host string allowance.".into());
        }
        self.state.copies(bytes)?;
        let mut text = String::new();
        text.try_reserve_exact(bytes)
            .map_err(|_| "Range UTF8 reservation failed.")?;
        text.extend(std::char::decode_utf16(output.iter().copied()).map(Result::unwrap));
        Ok(Value::String(text))
    }
}

#[path="script_range_comparisons.rs"]
mod comparisons;

#[path="script_static_range.rs"]
mod static_ranges;
